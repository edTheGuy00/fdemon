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
