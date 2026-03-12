## Task: iOS Tool Availability Checks

**Objective**: Add `idevicesyslog` availability checking to `ToolAvailability` and extend `native_logs_available()` to support the `"ios"` platform.

**Depends on**: None

### Scope

- `crates/fdemon-daemon/src/tool_availability.rs`: Add `idevicesyslog` field, check method, `"ios"` arm in `native_logs_available()`

### Details

#### 1. Add `idevicesyslog` field to `ToolAvailability`

```rust
/// Whether `idevicesyslog` is available (required for iOS physical device log capture).
/// Part of the `libimobiledevice` suite. Not needed for simulators (uses `xcrun simctl`).
#[cfg(target_os = "macos")]
pub idevicesyslog: bool,
```

#### 2. Add availability check method

`idevicesyslog` is a standalone binary from the `libimobiledevice` suite. Check availability via `idevicesyslog --help`:

```rust
/// Check if `idevicesyslog` is available on PATH.
///
/// Required for physical iOS device native log capture. Part of the
/// `libimobiledevice` suite, installable via `brew install libimobiledevice`.
/// Flutter also ships a bundled copy in its SDK cache.
///
/// Not needed for simulators — those use `xcrun simctl spawn log stream`.
#[cfg(target_os = "macos")]
async fn check_idevicesyslog() -> bool {
    Command::new("idevicesyslog")
        .arg("--help")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .inspect_err(|e| tracing::debug!("idevicesyslog check failed: {}", e))
        .unwrap_or(false)
}
```

**Note:** `idevicesyslog --help` exits with code 0 and prints usage. This is a reliable availability check unlike the macOS `log` command which always exits 64.

#### 3. Wire into `ToolAvailability::check()`

Add `check_idevicesyslog()` to the `tokio::join!` in `check()`:

```rust
pub async fn check() -> Self {
    let (xcrun_simctl, (android_emulator, emulator_path), adb) = tokio::join!(
        Self::check_xcrun_simctl(),
        Self::check_android_emulator(),
        Self::check_adb(),
    );

    #[cfg(target_os = "macos")]
    let (macos_log, idevicesyslog) = tokio::join!(
        Self::check_macos_log(),
        Self::check_idevicesyslog(),
    );

    Self {
        xcrun_simctl,
        android_emulator,
        emulator_path,
        adb,
        #[cfg(target_os = "macos")]
        macos_log,
        #[cfg(target_os = "macos")]
        idevicesyslog,
    }
}
```

#### 4. Extend `native_logs_available()` for iOS

iOS native log capture requires different tools depending on simulator vs physical:
- **Simulator**: Reuses `xcrun simctl` (already checked via `xcrun_simctl` field)
- **Physical device**: Requires `idevicesyslog`

For the `native_logs_available()` method, return `true` if **either** tool is available — the capture backend will select the right one at runtime:

```rust
pub fn native_logs_available(&self, platform: &str) -> bool {
    match platform {
        "android" => self.adb,
        #[cfg(target_os = "macos")]
        "macos" => self.macos_log,
        #[cfg(target_os = "macos")]
        "ios" => self.xcrun_simctl || self.idevicesyslog,
        _ => false,
    }
}
```

#### 5. Add `ios_native_log_tool()` helper

The app layer needs to know which specific tool to use (simulator vs physical). Add a helper that returns the available tool:

```rust
/// Determine which iOS native log capture tool is available for a given device.
///
/// - Simulators always use `xcrun simctl spawn log stream` (requires `xcrun_simctl`).
/// - Physical devices use `idevicesyslog` (requires the `libimobiledevice` tool).
///
/// Returns `None` if no suitable tool is available.
#[cfg(target_os = "macos")]
pub fn ios_native_log_tool(&self, is_simulator: bool) -> Option<IosLogTool> {
    if is_simulator && self.xcrun_simctl {
        Some(IosLogTool::SimctlLogStream)
    } else if !is_simulator && self.idevicesyslog {
        Some(IosLogTool::Idevicesyslog)
    } else {
        None
    }
}
```

Add the `IosLogTool` enum:

```rust
/// Available tools for iOS native log capture.
#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IosLogTool {
    /// `xcrun simctl spawn <udid> log stream` — for iOS simulators.
    SimctlLogStream,
    /// `idevicesyslog -u <udid> -p <process>` — for physical iOS devices.
    Idevicesyslog,
}
```

### Acceptance Criteria

1. `ToolAvailability` has `idevicesyslog: bool` field (cfg-gated to macOS)
2. `check_idevicesyslog()` runs `idevicesyslog --help` and returns `true` if it succeeds
3. `idevicesyslog` check runs in parallel with other tool checks via `tokio::join!`
4. `native_logs_available("ios")` returns `true` if either `xcrun_simctl` or `idevicesyslog` is available
5. `native_logs_available("ios")` returns `false` when neither tool is available
6. `ios_native_log_tool(is_simulator: true)` returns `Some(SimctlLogStream)` when `xcrun_simctl` is `true`
7. `ios_native_log_tool(is_simulator: false)` returns `Some(Idevicesyslog)` when `idevicesyslog` is `true`
8. `ios_native_log_tool(is_simulator: false)` returns `None` when `idevicesyslog` is `false`
9. All existing tests still pass — no regressions in `native_logs_available("android")` or `native_logs_available("macos")`
10. `cargo check -p fdemon-daemon` compiles
11. `IosLogTool` enum is exported from `fdemon-daemon`

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_logs_available_ios_with_simctl() {
        let tools = ToolAvailability {
            xcrun_simctl: true,
            android_emulator: false,
            emulator_path: None,
            adb: false,
            #[cfg(target_os = "macos")]
            macos_log: false,
            #[cfg(target_os = "macos")]
            idevicesyslog: false,
        };
        assert!(tools.native_logs_available("ios"));
    }

    #[test]
    fn test_native_logs_available_ios_with_idevicesyslog() {
        let tools = ToolAvailability {
            xcrun_simctl: false,
            android_emulator: false,
            emulator_path: None,
            adb: false,
            #[cfg(target_os = "macos")]
            macos_log: false,
            #[cfg(target_os = "macos")]
            idevicesyslog: true,
        };
        assert!(tools.native_logs_available("ios"));
    }

    #[test]
    fn test_native_logs_available_ios_no_tools() {
        let tools = ToolAvailability {
            xcrun_simctl: false,
            android_emulator: false,
            emulator_path: None,
            adb: false,
            #[cfg(target_os = "macos")]
            macos_log: false,
            #[cfg(target_os = "macos")]
            idevicesyslog: false,
        };
        assert!(!tools.native_logs_available("ios"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_ios_native_log_tool_simulator() {
        let tools = ToolAvailability {
            xcrun_simctl: true,
            idevicesyslog: true,
            ..Default::default()
        };
        assert_eq!(tools.ios_native_log_tool(true), Some(IosLogTool::SimctlLogStream));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_ios_native_log_tool_physical() {
        let tools = ToolAvailability {
            xcrun_simctl: true,
            idevicesyslog: true,
            ..Default::default()
        };
        assert_eq!(tools.ios_native_log_tool(false), Some(IosLogTool::Idevicesyslog));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_ios_native_log_tool_physical_no_idevicesyslog() {
        let tools = ToolAvailability {
            xcrun_simctl: true,
            idevicesyslog: false,
            ..Default::default()
        };
        assert_eq!(tools.ios_native_log_tool(false), None);
    }

    #[tokio::test]
    async fn test_check_idevicesyslog_does_not_panic() {
        let result = ToolAvailability::check_idevicesyslog().await;
        let _ = result;
    }
}
```

### Notes

- `idevicesyslog` is part of `libimobiledevice`, installable via `brew install libimobiledevice` on macOS. Flutter also bundles it in `$FLUTTER_ROOT/bin/cache/artifacts/libimobiledevice/`.
- `idevicesyslog` is confirmed broken on Xcode 26. This check will return `true` if the binary exists but capture may fail at runtime. The capture backend handles this gracefully (exits on error, sends `NativeLogCaptureStopped`).
- Simulators don't need `idevicesyslog` — they use `xcrun simctl spawn log stream` which is already gated by the existing `xcrun_simctl` check.
- The `IosLogTool` enum is needed by task 05 (app integration) to decide which config to build.
- All iOS tool fields/methods must be gated with `#[cfg(target_os = "macos")]` since iOS development only works from macOS.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/tool_availability.rs` | Added `adb` and `macos_log` fields (phase 1 prerequisites); added `idevicesyslog` field (cfg-gated to macOS); added `check_adb()`, `check_macos_log()`, `check_idevicesyslog()` methods; added `native_logs_available()` with android/macos/ios arms; added `ios_native_log_tool()` helper; added `IosLogTool` enum; added all task-specified tests |
| `crates/fdemon-daemon/src/lib.rs` | Added `#[cfg(target_os = "macos")] pub use tool_availability::IosLogTool` export; updated module doc comment |

### Notable Decisions/Tradeoffs

1. **Phase 1 prerequisites included**: The worktree was branched from `main` before phase 1 was merged. The task's acceptance criteria and test fixtures reference `adb`, `macos_log`, `check_adb()`, `check_macos_log()`, and `native_logs_available()` as existing infrastructure. These were added in the same commit as the phase 2 task 1 additions to make the worktree self-consistent.

2. **`IosLogTool` placed in `tool_availability.rs`**: The enum is defined at the top of the file (before `ToolAvailability`) so it is visible when `ToolAvailability::ios_native_log_tool()` references it in its return type — matching the task specification.

3. **`check_idevicesyslog` via `--help` exit code**: As specified in the task, `idevicesyslog --help` exits 0 on success, making exit-code-based detection reliable (unlike the macOS `log` command which always exits 64).

### Testing Performed

- `cargo check -p fdemon-daemon` — Passed
- `cargo test -p fdemon-daemon` — Passed (470 tests, 0 failed)
- `cargo clippy -p fdemon-daemon -- -D warnings` — Passed (no warnings)
- `cargo fmt --all` — Passed

### Risks/Limitations

1. **`idevicesyslog` broken on Xcode 26**: The availability check returns `true` if the binary is present and exits 0, but the tool itself may fail at runtime on Xcode 26. The task notes this is handled gracefully by the capture backend.
2. **Non-macOS iOS arm**: `native_logs_available("ios")` returns `false` on non-macOS targets due to cfg-gating, matching expected behaviour since iOS tooling is macOS-only.
