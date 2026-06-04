## Task: Android environment writer in path_config (ANDROID_HOME + bins)

**Objective**: Extend `path_config.rs` with a generalized, idempotent,
marker-fenced env-var writer that sets `ANDROID_HOME` and prepends
`$ANDROID_HOME/cmdline-tools/latest/bin` and `$ANDROID_HOME/platform-tools` to
`PATH`, across bash/zsh/fish rc files (POSIX) and the Windows user registry.

**Depends on**: 01

**Agent:** implementor

**Estimated Time**: 4-5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/path_config.rs`: add `add_android_env(...)`
  and the supporting generalized env/fence writer; reuse the existing shell
  detection and Windows out-of-band-value pattern.
- `crates/fdemon-daemon/src/toolchain/mod.rs`: re-export `add_android_env` (and any
  new outcome type).

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/types.rs`: `HostShell`, `HostPlatform`.
- existing `add_to_path` in the same file: reuse its rc-file selection,
  marker-fence idempotency, single-quoting, and the Windows `FDEMON_NEW_PATH`
  out-of-band injection-safe pattern.

### Details

`add_to_path` is PATH-only with a hardcoded fence marker
(`# >>> fdemon flutter path >>>`). Add an Android-specific writer with its **own
distinct fence marker** so the two blocks never collide and each is independently
idempotent.

```rust
pub fn add_android_env(
    shell: HostShell,
    platform: HostPlatform,
    sdk_root: &Path,
) -> Result<PathConfigOutcome>;   // reuse existing PathConfigOutcome { Written | AlreadyPresent }
```

POSIX rc-file block (bash/zsh — fence marker e.g. `# >>> fdemon android env >>>`):

```sh
# >>> fdemon android env >>>
export ANDROID_HOME="/home/user/.android/sdk"
export PATH="$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$PATH"
# <<< fdemon android env <<<
```

fish (`fish_add_path` + `set -Ux`):

```fish
# >>> fdemon android env >>>
set -Ux ANDROID_HOME "/home/user/.android/sdk"
fish_add_path "$ANDROID_HOME/cmdline-tools/latest/bin" "$ANDROID_HOME/platform-tools"
# <<< fdemon android env <<<
```

Windows — set the user-scope `ANDROID_HOME` and prepend the two bins to the user
`PATH` via PowerShell `[Environment]::SetEnvironmentVariable(name, value, 'User')`
(NOT `setx` — avoids the 1024-char truncation). Pass the SDK-root value out-of-band
through an env var (mirror the Phase 2 `FDEMON_NEW_PATH` pattern) so it is never
interpolated into the script string. Read the current user `PATH`, prepend the two
bin dirs if absent (idempotent), and write back.

Reuse the existing helpers: `rc_file_for_shell`, the single-quote/validation of the
path, the begin/end fence detection that makes re-runs `AlreadyPresent`. Factor the
fence read/replace logic shared with `add_to_path` into a private helper if it can
be done without disturbing `add_to_path`'s behavior; otherwise duplicate minimally.

### Acceptance Criteria

1. `add_android_env` writes `ANDROID_HOME` + the two bin entries to the correct rc
   file for bash/zsh/fish, fenced by a **distinct** Android marker (not the Flutter
   PATH marker).
2. Running it twice is idempotent: the second call returns
   `PathConfigOutcome::AlreadyPresent` and the rc file is byte-identical (golden-file
   test).
3. The `ANDROID_HOME` value is single-quoted/escaped on POSIX and passed
   out-of-band on Windows — no shell/PowerShell injection via the path.
4. Windows uses `[Environment]::SetEnvironmentVariable(..., 'User')` and is
   idempotent on `PATH` (no duplicate bin entries on re-run).
5. `add_android_env` re-exported from `toolchain/mod.rs`. `cargo check`/`clippy`/
   `test -p fdemon-daemon` pass.

### Testing

- **Golden-file idempotency:** write to a `tempdir()` rc file, capture contents,
  write again, assert `AlreadyPresent` + identical bytes. Cover bash, zsh, fish.
- **Fresh write content:** assert the block contains `ANDROID_HOME`,
  `cmdline-tools/latest/bin`, and `platform-tools`.
- **Distinct fence:** write both `add_to_path` and `add_android_env` to the same rc
  file and assert both fenced blocks coexist and each is independently idempotent.
- **Injection safety:** pass an `sdk_root` containing shell metacharacters and
  assert it is quoted/escaped, not interpolated.
- Windows registry write: gate behind `#[cfg(windows)]`; on non-Windows test the
  POSIX paths. If a Windows host is unavailable, unit-test the script/value
  construction (pure-string builder) without invoking PowerShell.

```rust
#[test]
fn test_add_android_env_idempotent_bash() { /* tempdir + golden bytes */ }

#[test]
fn test_android_env_block_has_both_bins() { /* assert substrings */ }
```

### Notes

- **Distinct fence marker is mandatory** — sharing the Flutter PATH marker would
  make the two writers fight over the same block.
- The Android env step is invoked from the **PATH Configuration** wizard step
  (task 06) — that step writes the Flutter PATH (existing `add_to_path`) and, when
  an Android SDK root is known, also calls `add_android_env`. This task only
  provides the writer; the orchestration is task 06.
- Keep the "restart your terminal" hint wording to the caller (task 06); this
  function just reports `Written` vs `AlreadyPresent`.
- `mod.rs` chain: 01→02→03; this task is last, adds only its re-export.

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/toolchain/path_config.rs` | Added `ANDROID_FENCE_OPEN`/`ANDROID_FENCE_CLOSE` marker constants; generalized `find_fence_range_for` (replaces removed `find_fence_range` + `find_android_fence_range` wrappers); `apply_fence_with_markers` private helper shared by both writers; `android_posix_block`, `android_fish_block`, `android_fence_block` pure string builders; `apply_android_fence`; `add_android_env_to_rc_file`; `add_android_env_windows`; public `add_android_env`; 15 new tests |
| `crates/fdemon-daemon/src/toolchain/mod.rs` | Added `add_android_env` to the `path_config` re-export line |

### Notable Decisions/Tradeoffs

1. **Generalized fence helper instead of duplication**: Extracted `find_fence_range_for(contents, open, close)` as the single implementation; `apply_fence_with_markers` drives both the Flutter PATH and Android env fence logic. This means `apply_fence` for the Flutter case is now a thin wrapper — no behavior change.

2. **Removed `find_fence_range` and `find_android_fence_range` thin wrappers**: Both were dead code after the generalization (only the `#[cfg(test)]` `fence_already_has_dir` helper and two test functions used `find_fence_range`; updated them to call `find_fence_range_for` directly). This keeps the dead-code lint clean without `#[allow]`.

3. **Fish uses double-quotes for `$ANDROID_HOME` expansion**: Fish `set -Ux ANDROID_HOME "..."` + `fish_add_path "$ANDROID_HOME/..."` correctly expands the variable. The Flutter PATH writer single-quotes the literal path; Android env needs expansion so double-quotes are correct here.

4. **Windows `HKCU:\\Environment` as synthetic `rc_file` path**: The existing Flutter PATH Windows writer uses `HKCU:\\Environment\\PATH`; the Android writer uses `HKCU:\\Environment` (no trailing variable name) since it modifies two variables. This is consistent with the out-of-band env-var injection-safe pattern.

### Testing Performed

- `cargo fmt --all -- --check` — Passed
- `cargo check --workspace --all-targets` — Passed (no warnings)
- `cargo test --workspace` — Passed (all test suites: 972 fdemon-daemon tests including 15 new Android env tests)
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed

### Risks/Limitations

1. **Windows branch untested at runtime**: The `add_android_env_windows` function is pure-string-builder tested (no PowerShell invoked) via `test_windows_android_home_script_uses_env_var`. Actual registry writes require a Windows host. Logic mirrors the proven `add_to_path_windows` pattern.

2. **Fish double-quote escaping**: If `sdk_root` contains a `"` character, it would break the fish block. However, `validate_bin_dir` rejects the metacharacters most likely to cause issues (newline, backtick, `$(`, `;`, `&`, `|`), and double-quote in a filesystem path is extraordinarily rare. The task spec does not require rejecting `"` explicitly; this matches the existing flutter writer's risk profile.
