# Task 02 — Reference-Returning Bootable Cache Accessor

**Agent:** implementor
**Phase:** 1
**Depends on:** none
**Files Modified (Write):**
- `crates/fdemon-app/src/state.rs`
- `crates/fdemon-app/src/handler/new_session/navigation.rs`

---

## Goal

Fix Major issue M2 from the review: `get_cached_bootable_devices()` currently clones two
`Vec`s on every call, asymmetric with `get_cached_devices()` which returns a borrow. The
clone should live at the single call site, mirroring the connected-device pattern.

## Context

Current accessor at `crates/fdemon-app/src/state.rs:1261-1266`:

```rust
pub fn get_cached_bootable_devices(&self) -> Option<(Vec<IosSimulator>, Vec<AndroidAvd>)> {
    match (&self.ios_simulators_cache, &self.android_avds_cache) {
        (Some(sims), Some(avds)) => Some((sims.clone(), avds.clone())),
        _ => None,
    }
}
```

Connected-device equivalent (post-Phase-1 of the parent plan):

```rust
pub fn get_cached_devices(&self) -> Option<&Vec<Device>> {
    self.device_cache.as_ref()
}
```

Sole caller of `get_cached_bootable_devices`: `crates/fdemon-app/src/handler/new_session/navigation.rs`,
inside `handle_open_new_session_dialog`, around line 221-234. The result is destructured
and forwarded to `set_bootable_devices(simulators, avds)` which takes ownership.

## Steps

1. Open `crates/fdemon-app/src/state.rs`. Update `get_cached_bootable_devices()` (around
   line 1261):

   ```rust
   pub fn get_cached_bootable_devices(&self) -> Option<(&Vec<IosSimulator>, &Vec<AndroidAvd>)> {
       match (&self.ios_simulators_cache, &self.android_avds_cache) {
           (Some(sims), Some(avds)) => Some((sims, avds)),
           _ => None,
       }
   }
   ```

   Update the doc comment to note that the caller is responsible for cloning if it needs
   ownership (mirroring the `get_cached_devices` doc).

2. Open `crates/fdemon-app/src/handler/new_session/navigation.rs`. Locate the
   single caller (around line 221). The pattern currently looks like:

   ```rust
   if let Some((simulators, avds)) = state.get_cached_bootable_devices() {
       // ... debug log
       state.new_session_dialog_state
           .target_selector
           .set_bootable_devices(simulators, avds);
       // ...
   }
   ```

   Update to clone at the call site, mirroring how `cached_devices.clone()` is used
   for the connected branch a few lines above:

   ```rust
   if let Some((simulators, avds)) = state.get_cached_bootable_devices() {
       let simulators = simulators.clone();
       let avds = avds.clone();
       // ... debug log (use simulators.len() / avds.len() before the moves if needed)
       state.new_session_dialog_state
           .target_selector
           .set_bootable_devices(simulators, avds);
       // ...
   }
   ```

   The borrow-checker may require capturing lengths into locals before the clones if the
   debug log uses `.len()`. Check the current code and adjust accordingly.

3. **Do not** change `set_bootable_devices` — it still takes ownership.

4. **Do not** add new tests. This is a pure refactor; existing tests
   (`test_open_dialog_uses_cached_devices`, `test_bootable_cache_*`, etc.) verify the
   behavior is unchanged.

5. Run verification:
   - `cargo fmt --all`
   - `cargo check -p fdemon-app`
   - `cargo test -p fdemon-app --lib`
   - `cargo clippy -p fdemon-app --lib -- -D warnings`

## Acceptance Criteria

- [ ] `get_cached_bootable_devices()` returns
      `Option<(&Vec<IosSimulator>, &Vec<AndroidAvd>)>` with no `.clone()` inside.
- [ ] Single call site in `navigation.rs` clones at the call site before forwarding to
      `set_bootable_devices`.
- [ ] No other call sites of `get_cached_bootable_devices()` exist (verify with grep).
- [ ] Existing bootable cache tests pass without modification.
- [ ] `cargo test -p fdemon-app --lib` passes (no regressions).
- [ ] `cargo clippy -p fdemon-app --lib -- -D warnings` clean.

## Out of Scope

- Changing `set_bootable_devices` to borrow instead of own (its caller paths in update
  handlers receive owned values from messages; ownership is correct there).
- Restructuring the bootable cache fields themselves.
- The cache-miss bootable discovery (handled in task 03).

---

## Completion Summary

**Status:** Done
**Branch:** main

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/state.rs` | Changed `get_cached_bootable_devices()` return type from `Option<(Vec<IosSimulator>, Vec<AndroidAvd>)>` to `Option<(&Vec<IosSimulator>, &Vec<AndroidAvd>)>`; removed `.clone()` calls inside the function; updated doc comment to note caller responsibility for cloning |
| `crates/fdemon-app/src/handler/new_session/navigation.rs` | Added `let simulators = simulators.clone()` and `let avds = avds.clone()` at the call site inside `handle_open_new_session_dialog`, before forwarding to `set_bootable_devices` |

### Notable Decisions/Tradeoffs

1. **Clone placement**: The clones are inserted immediately after the `if let` binding (before the debug log), so `simulators.len()` and `avds.len()` in the debug log refer to the freshly-cloned owned values. This matches the semantics of the original code and avoids any re-borrowing issues.

2. **No test changes**: The existing tests in `state.rs` (`test_get_cached_bootable_devices_valid`, `test_get_cached_bootable_devices_empty_when_not_set`) access the returned tuple via auto-deref, so they work unchanged with references.

### Testing Performed

- `cargo fmt --all` - Passed (no formatting changes needed)
- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app --lib` - Passed (1892 tests, 0 failures)
- `cargo clippy -p fdemon-app --lib -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **None**: Pure refactor with identical observable behavior. The borrow-checker enforces correctness at compile time.
