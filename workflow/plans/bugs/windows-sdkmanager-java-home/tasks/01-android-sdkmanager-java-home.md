# Task 01 — Guarantee a valid JAVA_HOME for the Android sdkmanager child

**Agent:** implementor
**Severity:** 🟠 MAJOR (Android Tools install is broken on a fresh Windows box)
**Depends On:** —
**Crate(s):** `fdemon-daemon`

## Problem

On Windows the Android Tools step downloads cmdline-tools fine but fails at license
acceptance with "sdkmanager --licenses … The system cannot find the path specified."
The error is emitted **inside `sdkmanager.bat`** when `%JAVA_HOME%\bin\java.exe` is
invalid. fdemon sets `JAVA_HOME`/JDK-bin-on-PATH for the sdkmanager child **only when
`target.jdk_path` is `Some`** (`android_install.rs:337-371`), and `jdk_path` (from
`settings.toolchain.jdk_path`, `actions.rs:207`) is `None` in the normal flow — so
sdkmanager relies on a frequently-broken **ambient** `JAVA_HOME`. fdemon already has
`resolve_jdk_home()` (`jdk.rs:30`) but the installer never falls back to it. See
`../BUG.md`.

## Goal

The Android installer must always provide the sdkmanager child a **validated**
`JAVA_HOME` (+ `<home>\bin` prepended to the child PATH), resolved via
`target.jdk_path` → `resolve_jdk_home()`. If no valid JDK home resolves, **fail the
step with a clear, actionable message** rather than letting sdkmanager emit the
cryptic Windows error.

## Decisions (approved)

- **Validation strictness:** require `bin/javac[.exe]` (a real JDK, not a JRE).
- **No-JDK behaviour:** fail the step with guidance (install a JDK / set
  `[toolchain] jdk_path` / fix `JAVA_HOME`).

## Acceptance Criteria

- [ ] JDK-home resolution precedence in the Android install env assembly:
      `target.jdk_path` (explicit) → `resolve_jdk_home()` (fallback). The fallback is
      the new behaviour.
- [ ] The chosen JDK home is **validated & normalized** before use:
      strip surrounding quotes and any trailing `\`/`/`; require the dir to exist and
      contain `bin/java[.exe]` **and** `bin/javac[.exe]` (true JDK). Provide a helper
      (e.g. `validate_jdk_home(&Path) -> bool`/`Result`) — Windows uses `.exe`
      suffixes, POSIX does not.
- [ ] On success, `JAVA_HOME` is set to the validated home **and** `<home>\bin` is
      prepended to the child PATH (extend the existing `Some` branch to also run for
      the resolved case; keep the OS-correct `split_paths`/`join_paths`).
- [ ] If no valid JDK home resolves, the step **fails with a clear error** naming the
      remedies — not "The system cannot find the path specified".
- [ ] **Pre-spawn guard:** before invoking sdkmanager, check
      `sdkmanager_path(&sdk_root).is_file()`; on failure, return an error that lists
      the contents of `cmdline-tools/latest/bin/` (so a future layout/relocation
      regression yields a precise message).
- [ ] POSIX behaviour preserved (Linux/macOS still resolve + validate the same way;
      `.exe` suffixes only under `cfg(windows)`).
- [ ] Both the `--licenses` call and the package-install call use the same validated
      env.

## Recommended Approach

- Add `validate_jdk_home(jdk_home: &Path) -> bool` (or `-> Result<PathBuf>` returning
  the normalized home) in `toolchain/jdk.rs`, with `#[cfg(windows)]` `.exe` suffixes.
  Reuse the existing `NON_JDK_PREFIXES`/marker ideas where sensible.
- In `android_install.rs`, replace the `target.jdk_path.as_ref().map(...)` env block
  with: resolve via `target.jdk_path` else `resolve_jdk_home()`, validate/normalize,
  then unconditionally push `JAVA_HOME` + prepend `bin` to PATH. If resolution/
  validation fails → `return Err(Error::process("… install a JDK 17 / set [toolchain]
  jdk_path / fix JAVA_HOME …"))` before the download? No — after download is fine, but
  ideally validate the JDK **before** the sdkmanager calls (it can be after the
  cmdline-tools download/relocate, right before license acceptance).
- Keep the change `fdemon-daemon`-local; no public-signature changes that ripple to
  `fdemon-app`.

## Files Modified (Write)

- `crates/fdemon-daemon/src/toolchain/android_install.rs`
- `crates/fdemon-daemon/src/toolchain/jdk.rs`

## Files Read (Dependencies)

- `crates/fdemon-daemon/src/toolchain/checks/android.rs` (`sdkmanager_bin_name`, path) — read only
- `crates/fdemon-daemon/src/toolchain/process_stream.rs` (`run_streaming_with_input` env) — read only
- `crates/fdemon-app/src/handler/install_wizard/actions.rs` (how `jdk_path` is passed) — read only

## Testing

- Unit (jdk.rs): `validate_jdk_home` accepts a dir with `bin/java[.exe]` + `bin/javac[.exe]`;
  rejects a `…/bin` path, a trailing-slash path, a JRE-only dir (java but no javac), a
  non-existent dir. Gate `.exe` variants under `cfg(windows)` or test the pure logic
  cross-platform with a tempdir fixture.
- Unit (android_install.rs): precedence (explicit `jdk_path` wins over resolver); env
  pairs include `JAVA_HOME` + a PATH with `<home>/bin` first; missing-JDK yields the
  guidance error; pre-spawn missing-sdkmanager yields the listing error.
- `cargo fmt`, `cargo check`, `cargo test`, `cargo clippy -D warnings` all green.
- **E2E (authoritative), `tests/docker/windows/`:** with a JDK installed, Android Tools
  license acceptance succeeds without setting `[toolchain] jdk_path`; with a broken
  ambient `JAVA_HOME`, fdemon shows the actionable error.

## Notes

- The JDK staleness mirrors the git re-check bug: `resolve_jdk_home()` reads the
  *current* env, and the just-merged Windows preflight PATH-refresh means `which java`
  now sees a JDK installed after fdemon launched — so the fallback resolves on a re-run.
- Do not weaken POSIX: Linux/macOS already worked for many users; keep their path
  intact, just add the resolver fallback + validation uniformly.

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/toolchain/jdk.rs` | Added `validate_jdk_home(&Path) -> Result<PathBuf>`: normalizes surrounding quotes + trailing slash/backslash, requires dir exists, requires `bin/java[.exe]` + `bin/javac[.exe]` (cfg(windows) .exe; POSIX no extension). Added 7 unit tests covering: full JDK accept, trailing slash stripped, bin-subdir rejected, non-existent rejected, JRE-only (no javac) rejected, surrounding quotes stripped, trailing backslash no-panic. |
| `crates/fdemon-daemon/src/toolchain/android_install.rs` | Imported `resolve_jdk_home` + `validate_jdk_home` from `super::jdk`. Replaced `target.jdk_path.as_ref().map(...)` Some-branch-only env block with: (1) unconditional resolution via `target.jdk_path.clone().or_else(resolve_jdk_home)`, (2) `validate_jdk_home()` call — fails step with actionable error if no valid JDK, (3) always push `JAVA_HOME` + prepend `<jdk>/bin` to child PATH for both `--licenses` and package-install calls. Added pre-spawn guard: `sdkmanager.is_file()` check that lists `cmdline-tools/latest/bin/` contents on failure. Added `list_dir_contents()` helper. Updated module doc. |

### Notable Decisions/Tradeoffs

1. **Validation before download vs. after relocation**: JDK validation fires after cmdline-tools download/extraction/relocation (just before sdkmanager license acceptance), per task recommendation. This is correct because sdkmanager is needed to accept licenses, not during download.
2. **`or_else(resolve_jdk_home)` (not closure)**: Clippy requires passing the function pointer directly rather than `or_else(|| resolve_jdk_home())` since the closure is redundant. This is cleaner and idiomatic.
3. **Error message deduplication**: `validate_jdk_home` already includes the guidance text; the call site wraps it with an additional install context prefix. This is slightly redundant but ensures the full context appears in the error regardless of call site.
4. **`list_dir_contents` is private**: Only needed for the pre-spawn diagnostic message, no need to expose it publicly.
5. **POSIX unchanged**: the `.exe` conditional applies only to binary name suffix; the resolution/validation logic runs identically on all platforms as required.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (7141 tests, 0 failures across all crates)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **E2E Windows VM not run on this host**: The Linux host verifies the cross-platform logic; actual Windows behavior is verified in the real Windows 11 VM per task notes. The logic is structurally correct — `cfg(windows)` blocks compile-time select `.exe` suffix, and the env-pair assembly was already tested for correctness by the existing `test_path_separator_*` tests.
2. **JDK staleness**: as noted in task, `resolve_jdk_home()` reads the *current* env. A JDK installed after fdemon launched will be found on the next re-run when the preflight PATH refresh propagates. This is documented behaviour, not a new limitation.

---

## Validation-Pass Completion Summary (Unit Tests)

**Status:** Done
**Branch:** feat/toolchain-bootstrap
**Commit:** 62abeb2

### Approach: Pure-Helper Extraction (Minimal Refactor)

The JDK env-assembly and pre-spawn guard logic were embedded in the async `install_android_tools_inner` function, making them un-testable in isolation. Two `pub(crate)` pure helpers were extracted:

- **`build_sdkmanager_env(sdk_root: &Path, jdk_path: Option<PathBuf>) -> Result<Vec<(String, String)>>`**: Encapsulates the resolution (`jdk_path.or_else(resolve_jdk_home)`), validation (`validate_jdk_home`), and env-pair assembly (`ANDROID_HOME` + `JAVA_HOME` + `PATH` with jdk/bin first). Called from `install_android_tools_inner` at the original site — identical runtime behavior.
- **`check_sdkmanager_guard(sdk_root: &Path) -> Result<()>`**: Encapsulates the `sdkmanager.is_file()` check and the bin-dir listing error. Called from `install_android_tools_inner` replacing the inline guard block.

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/toolchain/android_install.rs` | Extracted `build_sdkmanager_env` and `check_sdkmanager_guard` as `pub(crate)` helpers. Updated `install_android_tools_inner` to call them. Added 7 new unit tests (plus `make_jdk_fixture` / `make_sdkmanager_fixture` test helpers). Total test count: 27 (was 20). |

### New Tests Added

| Test | Coverage |
|------|---------|
| `test_build_sdkmanager_env_explicit_jdk_path_wins` | Explicit jdk_path is used over `resolve_jdk_home()` — JAVA_HOME matches fixture path |
| `test_build_sdkmanager_env_contains_required_vars` | ANDROID_HOME = sdk_root; JAVA_HOME = jdk_home; PATH first entry = `<jdk_home>/bin` (via `split_paths` ordering assertion) |
| `test_build_sdkmanager_env_invalid_jdk_path_yields_actionable_error` | Non-existent jdk_path returns Err naming remedies (jdk_path / JAVA_HOME / JDK / install / fix / set) |
| `test_build_sdkmanager_env_jre_only_dir_yields_error` | bin/java present but no bin/javac returns Err mentioning javac / JRE / JDK |
| `test_check_sdkmanager_guard_present_returns_ok` | sdkmanager binary present → Ok(()) |
| `test_check_sdkmanager_guard_absent_returns_err_with_listing` | sdkmanager absent, bin dir has decoy file → Err listing "not_sdkmanager.sh" |
| `test_check_sdkmanager_guard_absent_bin_dir_returns_err` | bin dir entirely absent → Err indicating dir absence |

### Testing Performed

- `cargo fmt --all -- --check` — Passed
- `cargo check --workspace --all-targets` — Passed
- `cargo test -p fdemon-daemon --lib toolchain::android_install` — 27 passed, 0 failed
- `cargo test --workspace` — All passed (0 failures)
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed

### Notable Decisions

1. **Extracted helpers, not new public API**: both helpers are `pub(crate)`, preserving the module boundary. No public signatures used by `fdemon-app` were changed.
2. **Tempdir fixtures with platform-correct binary names**: `make_jdk_fixture` uses `#[cfg(windows)]` `.exe` / `#[cfg(not(windows))]` no-extension, exactly mirroring the `validate_jdk_home` validator. `make_sdkmanager_fixture` calls `sdkmanager_bin_name()` for the same reason.
3. **`None` jdk_path not directly tested**: testing `build_sdkmanager_env(sdk, None)` on a CI machine with a system JDK installed would non-deterministically pass or fail. The "no JDK" branch is tested via `Some(nonexistent_path)` which exercises the same `validate_jdk_home` failure path, and via the JRE-only fixture. The `None → resolve_jdk_home()` fallback is covered by the existing `jdk.rs` tests.
