# Task 01 — Remove Device Cache TTL

**Agent:** implementor
**Phase:** 1
**Depends on:** none
**Files Modified (Write):** `crates/fdemon-app/src/state.rs`

---

## Goal

Remove the 30-second TTL from `get_cached_devices()` and `get_cached_bootable_devices()`
so the cached device lists survive for the lifetime of the `AppState`. The TTL gate is
the root cause of issue #33 — after 30s, accessors return `None` and the new-session
dialog falls into the cache-miss / loading branch even though the data is still in
memory.

## Steps

1. Open `crates/fdemon-app/src/state.rs`.

2. **`get_cached_devices()`** (around line 1239) — replace the TTL-gated body with a
   simple presence check:

   ```rust
   pub fn get_cached_devices(&self) -> Option<&Vec<Device>> {
       self.device_cache.as_ref()
   }
   ```

   Update the doc comment to reflect "cache survives for the lifetime of AppState; the
   dialog always triggers a background refresh on open to keep the list fresh."

3. **`get_cached_bootable_devices()`** (around line 1270) — same change. Return the
   tuple whenever both `ios_simulators_cache` and `android_avds_cache` are populated:

   ```rust
   pub fn get_cached_bootable_devices(&self) -> Option<(Vec<IosSimulator>, Vec<AndroidAvd>)> {
       match (&self.ios_simulators_cache, &self.android_avds_cache) {
           (Some(sims), Some(avds)) => Some((sims.clone(), avds.clone())),
           _ => None,
       }
   }
   ```

   Update the doc comment likewise.

4. **Keep** `devices_last_updated` and `bootable_last_updated`. They're still used by
   `tracing::debug!` in `handler/new_session/navigation.rs` to log cache age. Do not
   remove these fields or the `Instant::now()` writes in `set_device_cache` /
   `set_bootable_cache`.

5. **Update tests** in the inline `mod tests` block:
   - **Remove or rename** `test_device_cache_expires` (around line 1780). Replace with
     a positive test:

     ```rust
     #[test]
     fn test_device_cache_does_not_expire() {
         let mut state = AppState::new();
         state.set_device_cache(vec![test_device("dev1", "Device 1")]);

         // Simulate a stale timestamp — cache should still be returned.
         state.devices_last_updated =
             Some(std::time::Instant::now() - std::time::Duration::from_secs(60 * 60));
         assert!(state.get_cached_devices().is_some());
         assert_eq!(state.get_cached_devices().unwrap().len(), 1);
     }
     ```

   - Confirm `test_device_cache_fresh`, `test_device_cache_none_initially`,
     `test_device_cache_updates_timestamp`, and `test_device_cache_replaces_old`
     still pass.
   - If a `test_bootable_cache_expires` test exists, mirror the same change for it
     (rename to `_does_not_expire` and use a stale timestamp).

## Acceptance Criteria

- [ ] `get_cached_devices()` returns `Some(&Vec<Device>)` whenever
      `state.device_cache.is_some()`, regardless of `devices_last_updated`.
- [ ] `get_cached_bootable_devices()` returns `Some((sims, avds))` whenever both
      caches are populated, regardless of `bootable_last_updated`.
- [ ] `cargo test -p fdemon-app --lib` passes.
- [ ] No reference to `CACHE_TTL` or `Duration::from_secs(30)` remains in the two
      cache-getter functions.
- [ ] `devices_last_updated` / `bootable_last_updated` fields are preserved (still
      written by setters; still readable by tracing logs).

## Out of Scope

- Changing the dialog-open behaviour (handled in task 04).
- Adding the `refreshing` flags (handled in task 02).

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a62f90b7a5e7bf4cd

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/state.rs` | Removed TTL logic from `get_cached_devices()` and `get_cached_bootable_devices()`; updated doc comments; replaced `test_device_cache_expires` with `test_device_cache_does_not_expire` |
| `crates/fdemon-app/src/handler/new_session/navigation.rs` | Renamed `test_open_dialog_expired_cache_shows_loading` to `test_open_dialog_stale_timestamp_cache_still_shows_devices` and updated assertions to reflect new no-expiry behavior |

### Notable Decisions/Tradeoffs

1. **Navigation handler test update**: The test `test_open_dialog_expired_cache_shows_loading` in `navigation.rs` was also testing the old TTL behavior — it expected `loading = true` and a foreground `DiscoverDevices` action when cache had a stale timestamp. With the TTL removed, a populated cache (regardless of timestamp age) now yields an immediate display + background refresh. This test was updated accordingly. It was not explicitly listed in the task but was required to make the tests pass.

2. **Fields preserved**: `devices_last_updated` and `bootable_last_updated` remain in `AppState` and are still set by `set_device_cache()` / `set_bootable_cache()`. The tracing debug logs in the handler still reference them via `.map(|t| t.elapsed())`.

### Testing Performed

- `cargo test -p fdemon-app --lib` — Passed (1884 passed, 0 failed, 4 ignored)

### Risks/Limitations

1. **No TTL means stale data on reconnect**: The cache now survives indefinitely; freshness depends entirely on the background refresh triggered at dialog-open time. If a device disconnects between sessions, users may briefly see stale data until the background refresh completes. This is acceptable as it is the intended new design.
