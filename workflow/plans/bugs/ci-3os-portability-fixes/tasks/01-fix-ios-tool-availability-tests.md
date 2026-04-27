## Task: Gate iOS tool-availability tests to macOS

**Objective**: Add `#[cfg(target_os = "macos")]` to the two iOS-specific tests in `tool_availability.rs` so they only run on the platform where the production code path they exercise is compiled.

**Depends on**: None

**Estimated Time**: 0.25 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/tool_availability.rs`: Add `#[cfg(target_os = "macos")]` to `test_native_logs_available_ios_with_simctl` (around lines 379–386) and `test_native_logs_available_ios_with_idevicesyslog` (around lines 388–396).

**Files Read (Dependencies):**
- None — the production `native_logs_available` function in the same file already has the iOS arm gated `#[cfg(target_os = "macos")]`. The fix is purely test-side.

### Details

The production code defines:

```rust
pub fn native_logs_available(&self, platform: &str) -> bool {
    match platform {
        "android" => self.adb,
        #[cfg(target_os = "macos")]
        "ios" => self.xcrun_simctl || self.idevicesyslog,
        _ => false,
    }
}
```

On Linux and Windows the `"ios"` arm does not exist; the catch-all `_ => false` returns `false` for any iOS query. The two failing tests assert `native_logs_available("ios") == true` after setting iOS-specific fields. They must therefore be macOS-only:

```rust
#[test]
#[cfg(target_os = "macos")]
fn test_native_logs_available_ios_with_simctl() {
    let tools = ToolAvailability { xcrun_simctl: true, ..Default::default() };
    assert!(tools.native_logs_available("ios"));
}

#[test]
#[cfg(target_os = "macos")]
fn test_native_logs_available_ios_with_idevicesyslog() {
    let tools = ToolAvailability {
        #[cfg(target_os = "macos")]
        idevicesyslog: true,
        ..Default::default()
    };
    assert!(tools.native_logs_available("ios"));
}
```

Notes:
- The existing inner `#[cfg(target_os = "macos")]` on the `idevicesyslog` field literal stays as-is — it is independent of the new outer attribute on the test function.
- The companion `test_native_logs_available_ios_no_tools` (which asserts `false`) does **not** need the gate — `false` is what the catch-all returns on every platform, so the test correctly passes everywhere.

### Acceptance Criteria

1. Both `test_native_logs_available_ios_with_simctl` and `test_native_logs_available_ios_with_idevicesyslog` carry `#[cfg(target_os = "macos")]` in addition to their existing `#[test]` attribute.
2. `cargo test -p fdemon-daemon` passes on macOS (both tests run and assert true).
3. `cargo test -p fdemon-daemon --target x86_64-unknown-linux-gnu` (or just `cargo test -p fdemon-daemon` on Linux) excludes both tests from the run — they are no longer executed.
4. `cargo clippy -p fdemon-daemon --all-targets -- -D warnings` exits 0 on every platform.
5. `cargo fmt --all -- --check` is clean.
6. No other tests in `tool_availability.rs` are modified.

### Testing

Verify per-platform-target compilation by running:

```bash
cargo test -p fdemon-daemon tool_availability::tests::test_native_logs_available_ios
```

On macOS: 3 tests should run (both `_with_*` plus `_no_tools`).
On Linux/Windows: 1 test should run (`_no_tools` only) — the others are excluded by the cfg attribute.

### Notes

- This is the simplest correct fix. The alternative — making the production code report iOS availability on every platform via runtime checks — would mean dragging macOS-specific tooling logic into Linux/Windows binaries, which the existing cfg gate intentionally avoids.
- No production code changes.

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

- `cargo clippy -p fdemon-daemon --all-targets -- -D warnings` — _tbd_
- `cargo test -p fdemon-daemon` — _tbd_
- `cargo fmt --all -- --check` — _tbd_

### Risks/Limitations

_tbd_
