## Task: Fix `Instant` underflow in `test_device_cache_does_not_expire`

**Objective**: Replace the `Instant::now() - Duration::from_secs(3600)` line in `test_device_cache_does_not_expire` with a panic-safe form, so the test does not crash on freshly-booted Windows runners. Prefer the simplest fix that still exercises the cache contract.

**Depends on**: None

**Estimated Time**: 0.25 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/state.rs`: Edit `test_device_cache_does_not_expire` around lines 1778–1788.

**Files Read (Dependencies):**
- None — production behavior in `state.rs` is unchanged.

### Details

The test currently:

```rust
state.devices_last_updated =
    Some(std::time::Instant::now() - std::time::Duration::from_secs(60 * 60));
```

`Instant::now() - Duration::from_secs(3600)` panics with `overflow when subtracting duration from instant` when the system's monotonic clock value is less than 3600 seconds. Windows `Instant` ticks from boot; a freshly-booted GitHub Actions Windows runner panics here.

Background: `get_cached_devices()` returns `self.device_cache.as_ref()` with **no expiry check** (the cache never expires by design, per the test's name "does not expire"). The time manipulation in this test is therefore unnecessary — the contract being verified is "after `set_device_cache`, `get_cached_devices` returns `Some(_)` regardless of how stale the timestamp is."

#### Recommended fix (simplest)

Drop the time manipulation entirely. Verify the contract by setting the cache and reading it back without touching `devices_last_updated`:

```rust
// Verify: after set_device_cache, get_cached_devices returns Some — there is no expiry.
state.set_device_cache(vec![/* test devices */]);
assert!(state.get_cached_devices().is_some());
```

If the test was originally trying to demonstrate that even a one-hour-old timestamp does not invalidate the cache, the simpler form above expresses the same contract more honestly: it tests the absence of expiry logic rather than a specific stale-timestamp value.

#### Alternative fix (preserves time manipulation)

If the test is intentionally exercising the path where `devices_last_updated` is set to a stale value (e.g., for a future change where expiry might be added), use `checked_sub` with a fallback:

```rust
let stale_instant = std::time::Instant::now()
    .checked_sub(std::time::Duration::from_secs(60 * 60))
    .unwrap_or_else(std::time::Instant::now);
state.devices_last_updated = Some(stale_instant);
```

The `unwrap_or_else(now)` ensures the test still runs on freshly-booted Windows runners (with a non-stale timestamp), while preserving the original intent on systems with sufficient uptime.

**Pick the simplest form (recommended) unless reading the test reveals it must demonstrate stale-timestamp behavior.**

#### Sweep for similar patterns

Before completing, run a workspace-wide grep for other potential underflow sites:

```bash
grep -rn "Instant::now() *-" crates/
grep -rn "Instant::now() -" crates/
```

If any other test code subtracts a `Duration` from `Instant::now()`, apply `checked_sub` (or remove the manipulation if unnecessary). Production code should already use `Instant::now().elapsed()` rather than subtraction; verify quickly.

### Acceptance Criteria

1. `test_device_cache_does_not_expire` no longer contains a raw `Instant::now() - Duration::*` expression that can panic.
2. `cargo test -p fdemon-app state::tests::test_device_cache_does_not_expire` passes on macOS, and (verified by CI) on Linux and Windows.
3. `cargo clippy -p fdemon-app --all-targets -- -D warnings` exits 0.
4. `cargo test -p fdemon-app` passes.
5. `cargo fmt --all -- --check` is clean.
6. No production code in `state.rs` is modified.
7. Workspace-wide grep for `Instant::now() -` (with hyphen) shows no remaining underflow risks in test code.

### Testing

```bash
cargo test -p fdemon-app state::tests::test_device_cache_does_not_expire
```

This must pass on macOS. The Windows verification is via the post-merge CI matrix.

### Notes

- `tokio::time::pause` + `tokio::time::advance` is not appropriate here — the test does not use a tokio runtime, and the codebase does not use that pattern in `state.rs` tests.
- If the simpler "drop time manipulation" form is used, update the test's body comment to explain the contract being verified ("`get_cached_devices` has no expiry — calling it after `set_device_cache` always returns `Some`"). One short comment line is fine; more is unnecessary.

---

## Completion Summary

**Status:** Not Started
**Branch:** _to be filled by implementor_

### Files Modified

| File | Changes |
|------|---------|
| _tbd_ | _tbd_ |

### Notable Decisions/Tradeoffs

_tbd_

### Testing Performed

- `cargo clippy -p fdemon-app --all-targets -- -D warnings` — _tbd_
- `cargo test -p fdemon-app` — _tbd_
- `cargo fmt --all -- --check` — _tbd_

### Risks/Limitations

_tbd_
