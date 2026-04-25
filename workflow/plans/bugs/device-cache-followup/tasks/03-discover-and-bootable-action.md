# Task 03 — `DiscoverDevicesAndBootable` Action + Cache-Miss Wiring

**Agent:** implementor
**Phase:** 1
**Depends on:** 02 (file overlap on `navigation.rs`)
**Files Modified (Write):**
- `crates/fdemon-app/src/handler/mod.rs`
- `crates/fdemon-app/src/actions/mod.rs`
- `crates/fdemon-app/src/handler/new_session/navigation.rs`

---

## Goal

Fix Major issue M1 from the review: when both caches are empty, the cache-miss fallback
in `handle_open_new_session_dialog` dispatches only `UpdateAction::DiscoverDevices`, which
spawns connected discovery only. The bootable list silently sits at its default loading
state until the user manually switches tabs — contradicting the parent plan's milestone:
"Both connected and bootable lists are refreshed on every dialog open."

Add a new `UpdateAction::DiscoverDevicesAndBootable { flutter }` variant that runs
foreground connected discovery + background bootable discovery in parallel, mirroring
the existing `RefreshDevicesAndBootableBackground` pattern.

## Context

Cache-miss fallback at `crates/fdemon-app/src/handler/new_session/navigation.rs:258-261`:

```rust
state.new_session_dialog_state.target_selector.loading = true;
return UpdateResult::action(UpdateAction::DiscoverDevices { flutter });
```

`DiscoverDevices` action handler at `crates/fdemon-app/src/actions/mod.rs:75-77`:

```rust
UpdateAction::DiscoverDevices { flutter } => {
    spawn::spawn_device_discovery(msg_tx, flutter);
}
```

The existing `RefreshDevicesAndBootableBackground` variant at lines 85-90 spawns both
`spawn_device_discovery_background` (background) and `spawn_bootable_device_discovery`
(background). We need the same parallel-spawn shape but with the foreground connected
discovery so the user gets a proper loading indicator and surfaced errors.

## Steps

1. Open `crates/fdemon-app/src/handler/mod.rs`. Add a new variant to the `UpdateAction`
   enum, placed near `RefreshDevicesAndBootableBackground` (around line 80):

   ```rust
   /// Foreground connected-device discovery + background bootable discovery in parallel.
   /// Used by the new-session dialog cache-miss fallback so both tabs populate on
   /// first dialog open even when no caches exist.
   ///
   /// `UpdateResult` carries a single action, so this combined variant is the cleanest
   /// way to spawn both. Mirrors `RefreshDevicesAndBootableBackground` but uses the
   /// foreground (loading-aware) connected spawn.
   DiscoverDevicesAndBootable {
       /// Flutter executable to use for both discovery tasks.
       flutter: FlutterExecutable,
   },
   ```

2. Open `crates/fdemon-app/src/actions/mod.rs`. Add a match arm for the new action,
   placed near the existing `RefreshDevicesAndBootableBackground` arm (around line 85):

   ```rust
   UpdateAction::DiscoverDevicesAndBootable { flutter } => {
       spawn::spawn_device_discovery(msg_tx.clone(), flutter);
       spawn::spawn_bootable_device_discovery(msg_tx, tool_availability);
   }
   ```

   Notes:
   - Use `msg_tx.clone()` for the first spawn and let the second consume `msg_tx`
     (mirrors the `RefreshDevicesAndBootableBackground` pattern at lines 85-90).
   - `spawn_device_discovery` is the foreground variant (loading-aware, sends
     `DevicesDiscovered` on success and `DeviceDiscoveryFailed { is_background: false }`
     on failure).
   - `spawn_bootable_device_discovery` is the same one used by the background refresh
     (uses `tool_availability` for emulator/simulator listings).

3. Open `crates/fdemon-app/src/handler/new_session/navigation.rs`. Locate the cache-miss
   fallback in `handle_open_new_session_dialog` (around line 258-261). Replace:

   ```rust
   state.new_session_dialog_state.target_selector.loading = true;
   return UpdateResult::action(UpdateAction::DiscoverDevices { flutter });
   ```

   with:

   ```rust
   state.new_session_dialog_state.target_selector.loading = true;
   return UpdateResult::action(UpdateAction::DiscoverDevicesAndBootable { flutter });
   ```

   Do **not** change `loading = true` — the connected list still needs the foreground
   loading indicator. The bootable tab keeps its default `bootable_loading = true` from
   `TargetSelectorState::default()`, which `set_bootable_devices` will clear when the
   background discovery completes.

4. Add a unit test in the existing test module of `navigation.rs`. Place it near the
   other `handle_open_new_session_dialog` cache-miss tests:

   ```rust
   #[test]
   fn test_open_dialog_no_caches_dispatches_combined_discovery() {
       let mut state = AppState::new();
       // Both caches empty (default state)
       assert!(state.device_cache.is_none());
       assert!(state.ios_simulators_cache.is_none());
       assert!(state.android_avds_cache.is_none());

       let result = handle_open_new_session_dialog(&mut state, LoadedConfigs::default());

       assert!(state.new_session_dialog_state.target_selector.loading,
           "connected tab should show loading on cache miss");
       assert!(matches!(
           result.action,
           Some(UpdateAction::DiscoverDevicesAndBootable { .. })
       ), "cache miss should dispatch combined discovery, got {:?}", result.action);
   }
   ```

   If the existing cache-miss test
   (`test_open_dialog_cache_miss_shows_loading` or similar) asserted on
   `UpdateAction::DiscoverDevices`, **update its assertion** to match the new variant —
   do not delete it.

5. Verify exhaustive matching in `actions/mod.rs` — if there is a wildcard `_ => {}` arm,
   the new variant will silently fall through. Confirm via `cargo check`.

6. Run verification:
   - `cargo fmt --all`
   - `cargo check --workspace`
   - `cargo test -p fdemon-app --lib`
   - `cargo clippy --workspace --lib -- -D warnings`

## Acceptance Criteria

- [ ] `UpdateAction::DiscoverDevicesAndBootable { flutter: FlutterExecutable }` exists in
      `handler/mod.rs` with a doc comment and a `///` doc on the inner field.
- [ ] `actions/mod.rs` has a match arm that spawns both
      `spawn_device_discovery(msg_tx.clone(), flutter)` and
      `spawn_bootable_device_discovery(msg_tx, tool_availability)`.
- [ ] `navigation.rs` cache-miss fallback dispatches the new action instead of
      `DiscoverDevices`.
- [ ] New test `test_open_dialog_no_caches_dispatches_combined_discovery` is present and
      passes.
- [ ] Any existing cache-miss test that previously asserted on `DiscoverDevices` is
      updated to match the new variant.
- [ ] `cargo build --workspace` has no exhaustive-match warnings.
- [ ] `cargo test --workspace --lib` passes.
- [ ] `cargo clippy --workspace --lib -- -D warnings` clean.

## Out of Scope

- Changing the existing `DiscoverDevices` variant (still used elsewhere — e.g., engine
  startup). Verify with grep that other call sites are unaffected.
- Adding a generic multi-action `UpdateResult` mechanism (parent plan deferred this).
- Surfacing bootable discovery errors as a real `BootableDiscoveryFailed` message.
- Setting `bootable_loading = true` explicitly — the default state already has it set.
