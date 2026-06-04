## Task: Windows WM_SETTINGCHANGE broadcast + PATH error-path tests

**Objective**: After the Windows registry PATH/`ANDROID_HOME` write, broadcast
`WM_SETTINGCHANGE` so already-open processes pick up the change without restart; and
close the remaining PATH test gap (the `PowerShell`/`Cmd`/`Unknown` shell error path).

**Depends on**: None

**Estimated Time**: 1.5-2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/path_config.rs`: append a `WM_SETTINGCHANGE`
  broadcast to the Windows PowerShell PATH (`add_to_path_windows`, `:577`) and
  `ANDROID_HOME` (`add_android_env_windows`, `:654`) writers; add the
  `PowerShell/Cmd/Unknown` error-path tests.

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/types.rs`: `HostShell` / `HostPlatform`.

### Details

**Broadcast.** Research: `[Environment]::SetEnvironmentVariable(name, value, 'User')`
writes the registry correctly (already shipped, bypasses `setx`/1024-char limit) but
does **not** broadcast `WM_SETTINGCHANGE`, so open terminals/Explorer don't see the
change until restart. Append a `SendMessageTimeout(HWND_BROADCAST, WM_SETTINGCHANGE,
0, 'Environment', …)` P/Invoke via `Add-Type` after the set:

```powershell
# appended to the existing PowerShell script string
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class FdemonEnv {
  [DllImport("user32.dll", SetLastError=true, CharSet=CharSet.Auto)]
  public static extern IntPtr SendMessageTimeout(IntPtr hWnd, uint Msg, UIntPtr wParam,
      string lParam, uint fuFlags, uint uTimeout, out UIntPtr lpdwResult);
}
"@
[FdemonEnv]::SendMessageTimeout([IntPtr]0xFFFF, 0x1A, [UIntPtr]::Zero, "Environment", 2, 5000, [ref]([UIntPtr]::Zero)) | Out-Null
```

- `HWND_BROADCAST = 0xFFFF`, `WM_SETTINGCHANGE = 0x1A`, `SMTO_ABORTIFHUNG = 2`, 5s
  timeout. Keep it **out-of-band-safe**: the broadcast string `"Environment"` is a
  constant, and the variable value is still passed via `FDEMON_NEW_PATH` /
  `FDEMON_NEW_ANDROID_HOME` env vars (never interpolated) — preserve that.
- The `Add-Type` JIT cost (~once) is fine for an installer flow.
- A failed broadcast must **not** fail the PATH write — the registry value is already
  persisted; treat the broadcast as best-effort (`| Out-Null`, ignore errors).

**Folded test gap (from audit).** `add_to_path(HostShell::PowerShell/Cmd/Unknown, …)`
returns an `Err` with a manual-setup hint (`path_config.rs:194-200`) — currently
untested. Add tests for all three shells, symmetric for `add_android_env`.

### Acceptance Criteria

1. The Windows PATH and `ANDROID_HOME` writers append a best-effort
   `WM_SETTINGCHANGE` broadcast after the registry set; a broadcast failure does not
   fail the write.
2. The variable value remains passed out-of-band (no interpolation into the script);
   the existing anti-interpolation tests still pass.
3. `add_to_path` / `add_android_env` with `PowerShell`/`Cmd`/`Unknown` shells return
   an `Err` containing the manual-setup hint — now covered by tests.

### Testing

```rust
#[test]
fn add_to_path_powershell_shell_is_err_with_hint() {
    let err = add_to_path(HostShell::PowerShell, HostPlatform::Linux, &bin, /*…*/).unwrap_err();
    assert!(err.to_string().to_lowercase().contains("manual"));
}
// repeat for HostShell::Cmd, HostShell::Unknown, and for add_android_env

#[test]
fn windows_path_script_contains_broadcast_and_no_value_interpolation() {
    // build the script string for a Windows write; assert it contains
    // "WM_SETTINGCHANGE"/0x1A and references $env:FDEMON_NEW_PATH (not the literal path)
}
```

- The actual broadcast can't run on Linux CI — assert on the **generated script
  string** shape, and document the live broadcast as a manual verification step.

### Notes

- Independent of every other task (single file) — Wave 1, parallel with 01 and 05.
- Do not touch the bash/zsh/fish writers — they are complete and tested. Fish
  `conf.d` migration is **deferred** (out of scope).

---

## Completion Summary

**Status:** Not Started
**Branch:** feat/toolchain-bootstrap
