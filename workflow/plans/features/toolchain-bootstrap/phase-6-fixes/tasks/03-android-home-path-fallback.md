## Task: Write ANDROID_HOME + adb/emulator to PATH, with out-of-band fallback (Bug 4)

**Agent:** implementor

**Objective:** Ensure that after setup the user gets `adb`, `emulator`, and the
Android command-line tools in their terminal. Two gaps:

1. The Android PATH block omits `emulator/`, so the Android Emulator binary is not
   on PATH.
2. `ANDROID_HOME` is written only when the wizard's own AndroidTools step ran. If
   the user already has an Android SDK (set via `$ANDROID_HOME` / `$ANDROID_SDK_ROOT`
   or installed at the platform default), the PathConfig step silently skips the
   Android env block.

**Depends on:** — (file-disjoint from Tasks 01 and 02; safe in a parallel worktree)

**Estimated Time:** 3–4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/path_config.rs`
- `crates/fdemon-app/src/actions/mod.rs`
- `crates/fdemon-app/src/handler/install_wizard/actions.rs`

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/checks/android.rs` —
  `resolve_android_sdk_root_path(override: Option<&Path>) -> PathBuf` (read-only)

### Details

#### Fix 1 — add `emulator/` to the Android PATH block (`path_config.rs`)

`add_android_env` currently writes only `cmdline-tools/latest/bin` and
`platform-tools`. Add `$ANDROID_HOME/emulator` to all three writers:

- **POSIX block** (`android_posix_block`, ~line 364):
  ```sh
  export PATH="$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$ANDROID_HOME/emulator:$PATH"
  ```
- **Fish block** (`android_fish_block`, ~line 386):
  ```fish
  fish_add_path "$ANDROID_HOME/cmdline-tools/latest/bin" "$ANDROID_HOME/platform-tools" "$ANDROID_HOME/emulator"
  ```
- **Windows** (`add_android_env_windows`, ~line 727): add a third
  `{sdk}\emulator` entry to the idempotency check and the registry prepend loop.

`adb` lives in `platform-tools` (already present — confirm); `emulator` lives in
`$ANDROID_HOME/emulator`. Non-existent PATH entries are harmless (shells ignore
them), so add `emulator/` unconditionally for parity with the existing entries.
**Do not** add the deprecated `tools/bin/` (removed from the SDK in 26.0.0).

The block stays in its own distinct fence (`# >>> fdemon android env >>>` /
`# <<< fdemon android env <<<`), independent of the Flutter PATH fence, and the
existing in-place replace-on-change logic (`apply_android_fence`) is preserved.

#### Fix 2 — fall back to the resolver in the PathConfig executor (`actions/mod.rs`)

In the PathConfig executor (~`actions/mod.rs:1118-1186`), `add_android_env` is
called only `if let Some(sdk_root) = android_sdk_root`, where `android_sdk_root`
comes from `state.settings.toolchain.android_sdk_root` — populated only by a
successful wizard AndroidTools step. Add a fallback to the shared resolver before
the guard:

```rust
// Use the wizard-provided SDK root, else fall back to $ANDROID_HOME /
// $ANDROID_SDK_ROOT / platform default (same resolver the AndroidTools executor
// uses). Only write the Android env block if the resolved path actually exists.
let effective_android_root = android_sdk_root.or_else(|| {
    let p = fdemon_daemon::resolve_android_sdk_root_path(None);
    if p.is_dir() { Some(p) } else { None }
});
let android_outcome = if let Some(sdk_root) = effective_android_root {
    Some(add_android_env(shell, platform, &sdk_root)?)
} else {
    None
};
```

`resolve_android_sdk_root_path` is already re-exported at the `fdemon_daemon` crate
root (per ARCHITECTURE.md). Confirm the exact import path and use it.

#### Fix 3 — mirror the fallback at dispatch so the status tip is accurate (`handler/install_wizard/actions.rs`)

In `handle_run_selected_step` (~line 252), `android_sdk_root` is sourced from
settings only; when `None` a non-blocking tip is shown even if an SDK exists via
env vars. Apply the same `or_else(resolve_android_sdk_root_path(None) filtered by
is_dir())` fallback at dispatch so the tip fires only when **no** Android SDK exists
anywhere — keeping the dispatch-time message consistent with what the executor will
actually do.

> Scope guard: do **not** auto-persist the resolved root into
> `settings.toolchain.android_sdk_root` in this task (that would change the
> PersistSettings flow and the next re-run's AlreadyPresent semantics). Just resolve
> at use-time. The executor still returns `sdk_path: None` as today.

### Acceptance Criteria

1. After `add_android_env`, the written rc block (bash/zsh and fish) and the Windows
   registry prepend include **three** Android PATH dirs:
   `cmdline-tools/latest/bin`, `platform-tools`, and `emulator` — verified by a
   golden/string test per shell.
2. The Android env block remains idempotent and marker-fenced; re-running with a
   changed `sdk_root` replaces the block in place (existing behaviour preserved,
   now including the `emulator` entry).
3. When `settings.toolchain.android_sdk_root` is `None` but
   `resolve_android_sdk_root_path(None)` resolves to an existing directory
   (e.g. `$ANDROID_HOME` set, or the platform default exists), the PathConfig
   executor **writes** `ANDROID_HOME` (does not skip).
4. When no Android SDK exists anywhere (settings `None`, env unset, default absent),
   the executor skips the Android block and the dispatch-time tip is shown.
5. Flutter-PATH behaviour (`add_to_path`) and shell coverage (bash/zsh/fish/Windows;
   PowerShell/Cmd/Unknown return the manual-setup `Err`) are unchanged.

### Testing

```rust
// fdemon-daemon/src/toolchain/path_config.rs tests
// - UPDATE the android_posix_block / android_fish_block golden tests to assert the
//   "$ANDROID_HOME/emulator" entry is present (and ordering: cmdline-tools → platform-tools → emulator).
// - UPDATE the Windows add_android_env_windows idempotency test for the third dir.
//
// fdemon-app — PathConfig executor / dispatch tests
// - NEW: pathconfig_writes_android_env_from_resolver_when_settings_none
//        (set ANDROID_HOME to a temp dir that exists → add_android_env is called).
// - NEW: pathconfig_skips_android_env_when_no_sdk_anywhere.
// - KEEP: flutter-PATH-only behaviour unchanged.
```

### Notes

- The resolver returns a `PathBuf` even when the path does not exist; the `is_dir()`
  filter is what distinguishes "exists" from "default location that was never
  created". Keep that filter — writing `ANDROID_HOME` to a non-existent default
  would be misleading.
- `adb` is in `platform-tools` (already on PATH); this task's PATH addition is
  specifically the `emulator` binary plus the `ANDROID_HOME`-out-of-band fallback.
- File-disjoint from Tasks 01/02 — fully parallel.

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/toolchain/path_config.rs` | Added `$ANDROID_HOME/emulator` to POSIX and fish Android PATH blocks; updated Windows `add_android_env_windows` to check/add all 3 dirs (cmdline-tools, platform-tools, emulator); updated module doc comment; updated existing golden tests to assert emulator present; added 3 new tests (idempotency-requires-three-dirs, prepend-order, changed-sdk-root-includes-emulator) |
| `crates/fdemon-app/src/actions/mod.rs` | Added `or_else(resolve_android_sdk_root_path(None) filtered by is_dir())` fallback before the `add_android_env` guard in the PathConfig executor; added 2 new async executor tests (writes-android-env-from-resolver-when-settings-none, skips-android-env-when-no-sdk-anywhere) |
| `crates/fdemon-app/src/handler/install_wizard/actions.rs` | Mirrored the resolver fallback at dispatch time so the ordering tip fires only when no Android SDK exists anywhere; updated `test_pathconfig_hints_when_android_sdk_root_absent` to clear env vars + be robust to platform defaults; added `test_pathconfig_no_hint_when_android_home_env_set_to_existing_dir` |

### Notable Decisions/Tradeoffs

1. **Scope guard respected**: Did not persist the resolved `android_sdk_root` into `settings.toolchain.android_sdk_root` — the resolver is only called at use-time in both the executor and dispatch. This avoids changing the `PersistSettings` flow and `AlreadyPresent` semantics.
2. **Windows emulator dir prepend order**: Reversed insertion order (`emulator` first, then `platform_tools`, then `cmdline_bin`) so the final PATH is `cmdline-tools → platform-tools → emulator ...`, matching the POSIX block ordering.
3. **Idempotency preserved**: The Windows `AlreadyPresent` guard now requires all 3 dirs to be present; if only 2 were previously written, the function will update the PATH to add the missing `emulator` dir.

### Testing Performed

- `cargo fmt --all -- --check` — PASS
- `cargo check --workspace --all-targets` — PASS
- `cargo clippy --workspace --all-targets -- -D warnings` — PASS (fixed 2 `useless_vec` lints)
- `cargo test --workspace` — PASS (6862 tests across all crates, 0 failures)

### Risks/Limitations

1. **Non-existent emulator dir is harmless**: The `emulator/` dir may not exist for SDK-only installs; shells silently ignore non-existent PATH entries, so this is safe as documented.
2. **Serial test env-var isolation**: The two new async tests in `actions/mod.rs` and the updated dispatch test in `actions.rs` use `#[serial_test::serial]` to prevent ANDROID_HOME races — consistent with existing pattern in the codebase.
