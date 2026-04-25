# Task 04 — Dialog-Open Wiring (Set Refreshing Flags + Dispatch Combined Refresh)

**Agent:** implementor
**Phase:** 1
**Depends on:** 02 (refreshing flags), 03 (combined action variant)
**Files Modified (Write):** `crates/fdemon-app/src/handler/new_session/navigation.rs`

**Files Read:**
- `crates/fdemon-app/src/state.rs` (verify cache accessors from task 01)
- `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs` (use flags from
  task 02)
- `crates/fdemon-app/src/handler/mod.rs` (use UpdateAction variant from task 03)

---

## Goal

Update `handle_open_new_session_dialog()` so that opening the dialog:
1. Pre-populates the lists from the now-permanent cache (works after task 01).
2. Sets `target_selector.refreshing` and/or `bootable_refreshing` to `true`.
3. Dispatches the new `UpdateAction::RefreshDevicesAndBootableBackground` to refresh
   both lists in the background.

## Steps

1. Open `crates/fdemon-app/src/handler/new_session/navigation.rs`.

2. **Locate `handle_open_new_session_dialog()`** (around line 186).

3. **Replace the cache-hit / cache-miss control flow** at lines ~234-250 with the
   following structure:

   ```rust
   let connected_cached = has_connected_cache;
   let bootable_cached = state.get_cached_bootable_devices().is_some();
   //   ^--- recompute here OR refactor the bootable block above to also produce
   //        a `bootable_cached` boolean alongside the existing if-let.

   let Some(flutter) = state.flutter_executable() else {
       tracing::warn!(
           "handle_open_new_session_dialog: no Flutter SDK — skipping device refresh"
       );
       return UpdateResult::none();
   };

   if connected_cached || bootable_cached {
       // We have at least one cached list shown — refresh in the background.
       if connected_cached {
           state.new_session_dialog_state.target_selector.refreshing = true;
       }
       if bootable_cached {
           state.new_session_dialog_state.target_selector.bootable_refreshing = true;
       }
       return UpdateResult::action(
           UpdateAction::RefreshDevicesAndBootableBackground { flutter },
       );
   }

   // Both caches are empty — fall back to the foreground discovery path.
   tracing::debug!("Device cache miss, triggering foreground discovery");
   state.new_session_dialog_state.target_selector.loading = true;
   UpdateResult::action(UpdateAction::DiscoverDevices { flutter })
   ```

   Adjust the surrounding bootable-cache check (lines 219-232) so that
   `bootable_cached` is captured (mirror the existing `has_connected_cache` pattern):

   ```rust
   let bootable_cached = if let Some((simulators, avds)) = state.get_cached_bootable_devices() {
       tracing::debug!(
           "Using cached bootable devices ({} simulators, {} AVDs, age: {:?})",
           simulators.len(),
           avds.len(),
           state.bootable_last_updated.map(|t| t.elapsed())
       );
       state
           .new_session_dialog_state
           .target_selector
           .set_bootable_devices(simulators, avds);
       true
   } else {
       false
   };
   ```

   Note: `set_bootable_devices()` clears `bootable_loading` (and now `bootable_refreshing`
   per task 02). That's fine — we set `bootable_refreshing = true` **after**
   `set_bootable_devices()` runs. Order matters; verify by re-reading the final code.

4. **Update the inline doc comment** at lines 180-185 to reflect the new behaviour:

   ```rust
   /// Loads launch configurations from the project path and initializes
   /// the dialog state.
   ///
   /// Uses any cached devices/bootable for instant display. The cache has no TTL;
   /// it survives for the lifetime of the AppState. Whenever a cache hit occurs,
   /// the corresponding `refreshing` flag is set on the target selector and
   /// `RefreshDevicesAndBootableBackground` is dispatched so both lists stay
   /// fresh without a loading screen. If both caches are empty (first ever
   /// open), falls back to the foreground `DiscoverDevices` path.
   ```

5. **Update tests** in the inline `mod tests` block (around line 303 onward):

   - **Existing `test_open_dialog_uses_cached_devices`** — verify it still passes
     (cache hit path still pre-populates the list). Add an assertion that
     `state.new_session_dialog_state.target_selector.refreshing == true` after the
     call.

   - **Existing `test_open_dialog_triggers_background_refresh`** (or similar around
     line 351) — update the action-match arm from `RefreshDevicesBackground` to
     `RefreshDevicesAndBootableBackground`.

   - **New test:**

     ```rust
     #[test]
     fn test_open_dialog_sets_refreshing_flags_on_cache_hit() {
         let mut state = test_app_state();

         // Pre-populate both caches.
         state.set_device_cache(vec![test_device_full("1", "iPhone", "ios", false)]);
         state.set_bootable_cache(vec![], vec![]);

         let result = handle_open_new_session_dialog(&mut state);

         assert!(state.new_session_dialog_state.target_selector.refreshing);
         assert!(state.new_session_dialog_state.target_selector.bootable_refreshing);
         assert!(matches!(
             result.action,
             Some(UpdateAction::RefreshDevicesAndBootableBackground { .. })
         ));
     }

     #[test]
     fn test_open_dialog_only_connected_cached_sets_only_refreshing() {
         let mut state = test_app_state();
         state.set_device_cache(vec![test_device_full("1", "iPhone", "ios", false)]);

         let _ = handle_open_new_session_dialog(&mut state);
         assert!(state.new_session_dialog_state.target_selector.refreshing);
         assert!(!state.new_session_dialog_state.target_selector.bootable_refreshing);
     }

     #[test]
     fn test_open_dialog_no_caches_falls_back_to_loading() {
         let mut state = test_app_state();
         // No caches set.
         let result = handle_open_new_session_dialog(&mut state);
         assert!(state.new_session_dialog_state.target_selector.loading);
         assert!(!state.new_session_dialog_state.target_selector.refreshing);
         assert!(matches!(
             result.action,
             Some(UpdateAction::DiscoverDevices { .. })
         ));
     }
     ```

   - Verify that completing a `DevicesDiscovered` message clears `refreshing`. The
     existing `Message::DevicesDiscovered` handler in `update.rs` already calls
     `set_connected_devices()`, which (per task 02) clears `refreshing`. Add a focused
     test in `handler/tests.rs` if one doesn't already cover this:

     ```rust
     #[test]
     fn test_devices_discovered_clears_refreshing() {
         let mut state = test_app_state();
         state.show_new_session_dialog(LoadedConfigs::default());
         state.new_session_dialog_state.target_selector.refreshing = true;

         let _ = handler::update(&mut state, Message::DevicesDiscovered { devices: vec![] });
         assert!(!state.new_session_dialog_state.target_selector.refreshing);
     }
     ```

## Acceptance Criteria

- [ ] On dialog open with a connected-device cache hit, `refreshing` is set to `true`.
- [ ] On dialog open with a bootable cache hit, `bootable_refreshing` is set to `true`.
- [ ] When at least one cache is populated, the dialog dispatches
      `UpdateAction::RefreshDevicesAndBootableBackground`.
- [ ] When both caches are empty, the dialog falls back to setting `loading = true`
      and dispatching `UpdateAction::DiscoverDevices` (existing first-run behaviour
      preserved).
- [ ] `Message::DevicesDiscovered` clears `refreshing` (via `set_connected_devices`).
- [ ] `Message::BootableDevicesDiscovered` clears `bootable_refreshing` (via
      `set_bootable_devices`).
- [ ] All new tests in this file pass.
- [ ] `cargo test -p fdemon-app --lib` passes.

## Out of Scope

- Rendering the indicator (tasks 05 and 06).
- Modifying the cache accessors (task 01).
