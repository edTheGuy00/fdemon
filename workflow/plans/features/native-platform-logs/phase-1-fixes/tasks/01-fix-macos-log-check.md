## Task: Fix `check_macos_log()` — Always Returns `false`

**Objective**: Fix the macOS `log` command availability check so it correctly returns `true` when the `log` tool is present. Currently `log --help` exits with code 64 on macOS, causing `status.success()` to always return `false`.

**Depends on**: None

**Review Issue:** #1 (Critical/Blocking)

### Scope

- `crates/fdemon-daemon/src/tool_availability.rs`: Fix `check_macos_log()` function (lines 131-142)

### Details

#### Problem

The current implementation:

```rust
#[cfg(target_os = "macos")]
async fn check_macos_log() -> bool {
    Command::new("log")
        .arg("--help")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .inspect_err(|e| tracing::debug!("macOS log check failed: {}", e))
        .unwrap_or(false)
}
```

On macOS, `log --help` exits with code 64 (`EX_USAGE`). `s.success()` checks for exit code 0, so this always returns `false`. This is currently masked because `native_logs_available()` is dead code (issue #2), but will break macOS capture the moment the guard is wired in.

#### Fix

Change `--help` to `help` (no `--` prefix). macOS `log help` exits with code 0:

```rust
#[cfg(target_os = "macos")]
async fn check_macos_log() -> bool {
    Command::new("log")
        .arg("help")          // changed from "--help" — "log help" exits 0, "log --help" exits 64
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .inspect_err(|e| tracing::debug!("macOS log check failed: {}", e))
        .unwrap_or(false)
}
```

**Alternative considered:** `Command::new("which").arg("log")` — this works but is less robust (doesn't verify the binary is functional, just that it exists on PATH). The `log help` approach is consistent with how `check_adb` uses `adb version` to verify the tool works, not just that it exists.

### Acceptance Criteria

1. `check_macos_log()` returns `true` on macOS systems where `/usr/bin/log` exists
2. `ToolAvailability::check().macos_log` is `true` on macOS
3. `cargo test -p fdemon-daemon -- check_macos_log` passes
4. Existing `test_tool_availability_default_fields` and `test_native_logs_available_*` tests still pass
5. `cargo clippy -p fdemon-daemon -- -D warnings` passes

### Testing

The existing test `test_check_macos_log_does_not_panic` (tool_availability.rs) already runs `check_macos_log()` and verifies no panic. After the fix, add an assertion that it returns `true` on macOS CI:

```rust
#[cfg(target_os = "macos")]
#[tokio::test]
async fn test_check_macos_log_returns_true_on_macos() {
    // On macOS, the `log` command is always present (since Sierra 10.12).
    let result = check_macos_log().await;
    assert!(result, "check_macos_log() should return true on macOS");
}
```

### Notes

- This is a **blocking** fix — must be completed before task 02 can wire the tool availability guard.
- The existing `test_check_macos_log_does_not_panic` test passes on macOS but doesn't assert the return value. The task noted this risk in its completion summary: *"`macos_log` check returns `false` on `log --help` exit code"*.
- Single-line change: `"--help"` → `"help"`.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/tool_availability.rs` | Replaced subprocess-based `check_macos_log()` with a path existence check; added `test_check_macos_log_returns_true_on_macos` test |

### Notable Decisions/Tradeoffs

1. **Path existence check instead of `log help`**: The task specified changing `--help` to `help`, but on macOS 26.3 (the environment in use) every invocation of `/usr/bin/log` exits with code 64 regardless of arguments — including `log help`. Switching to `std::path::Path::new("/usr/bin/log").exists()` is the most reliable approach: the binary is at a fixed canonical path, has been there since macOS 10.12 Sierra, and path existence is not subject to exit code variation across macOS versions. The doc comment explains the rationale inline.

2. **Unused imports**: The `Command`/`Stdio` imports from `tokio::process` and `std::process` remain because other functions in the same `impl` block still use them — clippy confirms no warnings.

### Testing Performed

- `cargo check -p fdemon-daemon` - Passed
- `cargo test -p fdemon-daemon -- check_macos_log --nocapture` - Passed (1 test)
- `cargo test -p fdemon-daemon -- tool_availability --nocapture` - Passed (13 tests)
- `cargo clippy -p fdemon-daemon -- -D warnings` - Passed (no warnings)
- `cargo fmt --all && cargo check --workspace` - Passed

### Risks/Limitations

1. **Path hardcoded to `/usr/bin/log`**: This is the canonical, stable path for the macOS unified logging tool and is appropriate for this platform-specific (`#[cfg(target_os = "macos")]`) function. If Apple ever relocates the binary this would need updating, but that is extremely unlikely.
