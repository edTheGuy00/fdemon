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

**Status:** Done
**Branch:** fix/detect-windows-bat

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/tool_availability.rs` | Added `#[cfg(target_os = "macos")]` attribute to `test_native_logs_available_ios_with_simctl` and `test_native_logs_available_ios_with_idevicesyslog` tests |

### Notable Decisions/Tradeoffs

1. **Minimal change**: Only the two outer test attributes were added. The inner `#[cfg(target_os = "macos")]` on the `idevicesyslog` struct field literal inside `test_native_logs_available_ios_with_idevicesyslog` was left as-is, as it is independent of the new outer attribute and still required for the struct initializer to compile on non-macOS platforms.
2. **No production code changes**: Exactly as the task specifies — the fix is purely on the test side.
3. **`test_native_logs_available_ios_no_tools` unchanged**: That test asserts `false` (which the catch-all `_ => false` returns on every platform), so it correctly passes everywhere and needs no gate.

### Testing Performed

- `cargo test -p fdemon-daemon tool_availability::tests` — Passed (20 tests, on macOS both gated tests ran and asserted true)
- `cargo clippy -p fdemon-daemon --all-targets -- -D warnings` — Passed (0 warnings)
- `cargo fmt --all -- --check` — Passed (clean)

### Risks/Limitations

1. **Linux/Windows verification**: The CI will confirm the gated tests are excluded on non-macOS platforms. On macOS (local dev environment), all 20 tests run. The `#[cfg]` attribute correctly gates compilation so neither test appears in the binary on Linux or Windows.
