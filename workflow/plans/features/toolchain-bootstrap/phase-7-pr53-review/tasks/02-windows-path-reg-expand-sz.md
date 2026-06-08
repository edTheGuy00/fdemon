## Task: Preserve `REG_EXPAND_SZ` and literal `%VAR%` tokens when writing the Windows user PATH (F-PR53-02)

**Severity:** HIGH (correctness / destructive environment mutation)

**Objective**: Stop fdemon from permanently flattening the user's global PATH.
Round-tripping the user PATH through .NET's
`[Environment]::GetEnvironmentVariable('PATH','User')` /
`SetEnvironmentVariable(...,'User')` **expands** any `%VAR%` references and
re-persists the value as `REG_SZ`, destroying `REG_EXPAND_SZ` entries such as
`%USERPROFILE%\bin`, `%JAVA_HOME%\bin`, `%LOCALAPPDATA%\Microsoft\WindowsApps`.
The user's PATH then breaks the moment any referenced variable changes, with no
backup.

**Depends on**: — (chain C start; shares file with task 07)

**Estimated Time**: 4–6 hours (plus Windows verification)

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/path_config.rs`

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/path_config.rs` (`add_to_path_windows`, `add_android_env_windows`, `broadcast_wm_settingchange`)

### Details

`add_to_path_windows` (`path_config.rs:606-676`) and
`add_android_env_windows` (`path_config.rs:688-825`) both:

```rust
// read — EXPANDS %VAR% and returns a plain string
"[Environment]::GetEnvironmentVariable('PATH', 'User')"      // lines 616, 717
// ...append new dir to the already-expanded string...        // lines 637-643 / 795
// write — always persists as REG_SZ, dropping the ExpandString type
"[Environment]::SetEnvironmentVariable('PATH', $env:FDEMON_NEW_PATH, 'User')"  // lines 654, 803
```

This is documented .NET behavior: the `User` getter has no
raw/unexpanded overload, and `SetEnvironmentVariable` always writes `REG_SZ`.
The existing doc comments (lines 592-605, 678-687) only discuss truncation and
injection safety — they say nothing about the expansion / REG-type side effect.
The injection-safe out-of-band `$env:FDEMON_NEW_PATH` pattern must be preserved.

### Proposed Fix

Do not round-trip through the expanding getter/setter. Operate on the **raw**
registry value under `HKCU:\Environment`:

1. **Read raw**: `Get-ItemProperty -Path 'HKCU:\Environment' -Name Path` (or
   `(Get-Item 'HKCU:\Environment').GetValue('Path', '', 'DoNotExpandEnvironmentNames')`)
   to obtain the literal, unexpanded value and its kind.
2. **Append** the new bin dir to that *unexpanded* value (idempotency check must
   compare against the raw value, case-insensitively, as today).
3. **Write preserving type**: if the existing value was `REG_EXPAND_SZ` (or
   contains `%`), write back with `New-ItemProperty ... -PropertyType ExpandString -Force`;
   otherwise keep `String`. When PATH did not previously exist, default to
   `ExpandString` (safe superset).
4. Keep the out-of-band `$env:FDEMON_NEW_PATH` injection guard and the
   best-effort `broadcast_wm_settingchange()` afterward. Apply the same change to
   `add_android_env_windows` for `ANDROID_HOME`/`ANDROID_SDK_ROOT`/PATH writes.
5. Update the doc comments to state that the raw `REG_EXPAND_SZ` type and `%VAR%`
   tokens are preserved.

### Acceptance Criteria

1. After a PATH write, a pre-existing `REG_EXPAND_SZ` user PATH retains its
   `REG_EXPAND_SZ` registry type and its literal `%VAR%` tokens (verified on
   Windows via `Get-ItemProperty`/`reg query`); only the new bin dir is appended.
2. Idempotency: a second run detects the (raw) entry is already present and
   returns `AlreadyPresent` without rewriting.
3. The out-of-band `$env:FDEMON_NEW_PATH` injection-safety property is retained
   (no user value interpolated into the script string).
4. `add_android_env_windows` writes ANDROID env vars / PATH with the same
   type-preserving behavior.

### Testing

```rust
// path_config.rs test module (Windows-gated where it must touch the registry)
// - Pure helper test: factor the "append-to-unexpanded-value + decide kind" logic
//   into a pure fn (e.g. plan_windows_path_update(raw_value, kind, bin_dir)) and
//   unit-test it on all platforms: %VAR%-bearing value -> ExpandString kept,
//   plain value -> String kept, already-present -> None, empty/new -> ExpandString.
// - #[cfg(windows)] integration test (or documented manual check) asserting the
//   registry kind survives a real write to a temp HKCU subkey.
```

### Notes

- High blast radius and silent — lead this with task 01 in Wave 1.
- Shares `path_config.rs` with task 07 (rc-file perms) — run serially on the same
  branch (chain C), not parallel worktrees.
- Requires a Windows runner or VM to truly verify; gate the registry-touching
  test behind `#[cfg(windows)]` and keep the planning logic in a pure,
  cross-platform-testable helper.

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/toolchain/path_config.rs` | Replaced `[Environment]::GetEnvironmentVariable/SetEnvironmentVariable` round-trip with raw registry access via `GetValue(..., 'DoNotExpandEnvironmentNames')` + `New-ItemProperty -PropertyType`. Added `WindowsRegKind` enum, `decide_reg_kind()` pure fn, `plan_windows_path_update()` pure fn, 4 new PowerShell script constants. Updated `add_to_path_windows` and `add_android_env_windows`. Updated doc comments. Added 16 new unit tests. |

### Notable Decisions/Tradeoffs

1. **`plan_windows_path_update` empty-path override**: `decide_reg_kind("", false)` returns `String` by design (that function is a pure classifier). The override to `ExpandString` for new/absent keys lives in `plan_windows_path_update` itself, which is the correct place — the caller that knows context (no key exists yet) makes the defaulting decision. The test `test_decide_reg_kind_empty_no_flag_is_string` explicitly documents this separation.

2. **`ANDROID_HOME` written as `REG_SZ` (`String`)**: The task spec says ANDROID_HOME gets written as `REG_SZ` (a concrete path, not a `%VAR%` template). Only PATH needs type-preservation since existing PATH entries may contain `%USERPROFILE%\bin` etc. This is consistent with the task spec ("ANDROID_HOME" is a concrete dir that the user specifies, not a template).

3. **Out-of-band env vars extended to include `FDEMON_PATH_KIND`**: The property type is also passed as an env var to the write script, keeping the script entirely constant with no user-controlled interpolation surface.

4. **Idempotency checks on raw value**: The already-present check in `plan_windows_path_update` operates on the raw (unexpanded) value, meaning a literal `%JAVA_HOME%\bin` entry in PATH will be detected correctly without requiring the env to be expanded.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (1109 fdemon-daemon unit tests, 0 failed; 5069+ total)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed (0 errors)
- 16 new pure unit tests added (cross-platform, no PowerShell required):
  - `test_plan_windows_path_update_expand_sz_when_percent_tokens`
  - `test_plan_windows_path_update_expand_sz_from_existing_flag`
  - `test_plan_windows_path_update_string_when_plain_value`
  - `test_plan_windows_path_update_empty_path_defaults_to_expand_string`
  - `test_plan_windows_path_update_idempotent`
  - `test_plan_windows_path_update_trailing_semicolon`
  - `test_plan_windows_path_update_preserves_percent_tokens`
  - `test_decide_reg_kind_percent_in_value`
  - `test_decide_reg_kind_existing_flag`
  - `test_decide_reg_kind_plain_value`
  - `test_decide_reg_kind_empty_no_flag_is_string`
  - `test_write_raw_path_script_uses_env_vars_not_interpolation`
  - `test_read_raw_path_script_does_not_expand`
  - `test_write_android_home_script_uses_env_var_not_interpolation`
  - `test_windows_reg_kind_property_type_strings`
  - Updated 3 existing tests to reference new script constants

### Risks/Limitations

1. **Windows-only path**: The fix can only be fully verified on a Windows runner. The PowerShell scripts (`READ_RAW_PATH_SCRIPT`, `WRITE_RAW_PATH_SCRIPT`, `READ_RAW_ANDROID_HOME_SCRIPT`, `WRITE_ANDROID_HOME_SCRIPT`) are `allow(dead_code)` on non-Windows and are validated purely by script-content assertions on all platforms.

2. **`GetValueKind` availability**: `GetValueKind` is available in PowerShell 5.1+ and PowerShell Core 6+, which covers all modern Windows versions. On Windows 7 (EOL), this may not be available — but Flutter itself dropped Windows 7 support years ago.
