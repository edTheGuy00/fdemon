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
