# Task 02 — Add Refreshing State Flags to TargetSelectorState

**Agent:** implementor
**Phase:** 1
**Depends on:** none
**Files Modified (Write):** `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs`

---

## Goal

Introduce two boolean flags on `TargetSelectorState` that indicate whether a background
device-list refresh is in flight on each tab. Task 04 sets these on dialog open; the
`set_*_devices` / `set_error` methods clear them on completion. Task 06 wires them into
the `TabBar` widget for rendering.

## Steps

1. Open `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs`.

2. **Add fields** to the `TargetSelectorState` struct (after `bootable_loading`, around
   line 42):

   ```rust
   /// Background refresh in progress for connected devices.
   ///
   /// Distinct from `loading`: `loading` shows a full-screen spinner with no
   /// content, whereas `refreshing` is set when the cached list is already shown
   /// and a background discovery is updating it in place. Cleared by
   /// `set_connected_devices()` and `set_error()`.
   pub refreshing: bool,

   /// Background refresh in progress for bootable devices.
   ///
   /// Mirror of `refreshing` for the bootable tab. Cleared by
   /// `set_bootable_devices()`.
   pub bootable_refreshing: bool,
   ```

3. **Update `Default::default()`** (around line 62) — initialize both flags to `false`:

   ```rust
   refreshing: false,
   bootable_refreshing: false,
   ```

4. **Update `set_connected_devices()`** (around line 212) — clear `refreshing`:

   ```rust
   pub fn set_connected_devices(&mut self, devices: Vec<Device>) {
       self.connected_devices = devices;
       self.loading = false;
       self.refreshing = false;       // NEW
       self.error = None;
       // ... existing body
   }
   ```

5. **Update `set_bootable_devices()`** (around line 229) — clear `bootable_refreshing`:

   ```rust
   pub fn set_bootable_devices(
       &mut self,
       ios_simulators: Vec<IosSimulator>,
       android_avds: Vec<AndroidAvd>,
   ) {
       self.ios_simulators = ios_simulators;
       self.android_avds = android_avds;
       self.bootable_loading = false;
       self.bootable_refreshing = false;   // NEW
       // ... existing body (keep the `error` not-cleared comment)
   }
   ```

6. **Update `set_error()`** (around line 254) — clear `refreshing` (the user-visible
   error supersedes the in-progress hint):

   ```rust
   pub fn set_error(&mut self, error: String) {
       self.error = Some(error);
       self.loading = false;
       self.refreshing = false;       // NEW
   }
   ```

   Note: do **not** clear `bootable_refreshing` here — `set_error` is currently used
   for SDK/connected-side errors only; bootable errors go via a different path. If
   investigation shows `set_error` is also used for bootable failures, clear both.

7. **Add unit tests** (in the inline `mod tests` block):

   ```rust
   #[test]
   fn test_refreshing_default_false() {
       let state = TargetSelectorState::default();
       assert!(!state.refreshing);
       assert!(!state.bootable_refreshing);
   }

   #[test]
   fn test_set_connected_devices_clears_refreshing() {
       let mut state = TargetSelectorState::default();
       state.refreshing = true;
       state.set_connected_devices(vec![]);
       assert!(!state.refreshing);
   }

   #[test]
   fn test_set_bootable_devices_clears_bootable_refreshing() {
       let mut state = TargetSelectorState::default();
       state.bootable_refreshing = true;
       state.set_bootable_devices(vec![], vec![]);
       assert!(!state.bootable_refreshing);
   }

   #[test]
   fn test_set_error_clears_refreshing() {
       let mut state = TargetSelectorState::default();
       state.refreshing = true;
       state.set_error("boom".to_string());
       assert!(!state.refreshing);
   }
   ```

## Acceptance Criteria

- [ ] `TargetSelectorState` has two new public fields: `refreshing: bool` and
      `bootable_refreshing: bool`.
- [ ] Default values are `false` for both.
- [ ] `set_connected_devices()` clears `refreshing`.
- [ ] `set_bootable_devices()` clears `bootable_refreshing`.
- [ ] `set_error()` clears `refreshing`.
- [ ] All four new unit tests pass.
- [ ] `cargo test -p fdemon-app --lib` passes (existing tests should still pass —
      the new fields default to `false`, matching prior behaviour).

## Out of Scope

- Setting the flags `true` (handled in task 04).
- Rendering the indicator (handled in tasks 05 and 06).

---

## Completion Summary

**Status:** Done
**Branch:** main

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs` | Added `refreshing` and `bootable_refreshing` fields to struct and `Default`; cleared `refreshing` in `set_connected_devices()` and `set_error()`; cleared `bootable_refreshing` in `set_bootable_devices()`; added 4 new unit tests |

### Notable Decisions/Tradeoffs

1. **Field placement**: The two new fields are placed after `bootable_loading` and before `error` in the struct definition, grouping the loading/refreshing indicators together logically.
2. **`set_error` scope**: Only `refreshing` is cleared (not `bootable_refreshing`), per the task spec — `set_error` is used for SDK/connected-side errors only.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check --workspace` - Passed
- `cargo test -p fdemon-app --lib` - Passed (1888 tests, 0 failed)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **Flags not yet set to `true`**: Setting these flags is intentionally out of scope (task 04). Until task 04 is implemented, the flags will always remain `false` at runtime.
