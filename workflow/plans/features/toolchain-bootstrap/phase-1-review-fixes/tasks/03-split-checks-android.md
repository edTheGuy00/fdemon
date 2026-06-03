## Task: Split checks.rs Into an Android Submodule (fdemon-daemon)

**Objective**: Bring `toolchain/checks.rs` (962 lines, ~2× the 500-line standard) under the limit by
extracting the Android-specific probes into a dedicated submodule. Pure refactor — no behavior
change. Addresses review finding **m7**.

**Depends on**: 02-checks-correctness-ansi-test-isolation (same file; split after correctness edits
land to avoid merge conflicts)

**Agent:** implementor

**Estimated Time**: 2-3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/checks.rs` — remove the Android functions (now re-exported
  from the submodule); keep Flutter/git/JDK/prerequisites probes.
- `crates/fdemon-daemon/src/toolchain/checks/android.rs` (NEW) — the Android probes.
- `crates/fdemon-daemon/src/toolchain/checks/mod.rs` (NEW, **only if** converting `checks.rs` to a
  directory module — see Module Structure for the chosen layout).

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/mod.rs` — confirms which `check_*` symbols `run_preflight`
  calls, so the public-within-crate surface is preserved.
- `crates/fdemon-daemon/src/toolchain/types.rs` — shared types used by the moved functions.

### Module Structure

Two acceptable layouts — pick the one that keeps imports simplest and **document the choice** in the
completion summary:

- **Option A (sibling file):** keep `checks.rs` as the module root and add a sibling
  `checks_android.rs`? — NOT preferred (Rust requires the submodule file to live in a `checks/`
  directory when `checks` is a file module). Use Option B.
- **Option B (directory module, preferred):** convert `checks.rs` → `checks/mod.rs`, add
  `checks/android.rs`. In `checks/mod.rs`: `mod android; pub(crate) use android::*;` (or name the
  specific re-exports). Move `check_android_cmdline_tools`, `check_android_platform_tools`,
  `check_android_platform`, `check_android_build_tools`, `check_android_licenses`,
  `android_sdk_root`, `platform_default_android_sdk`, `sdkmanager_bin_name`, `count_subdirs`, the
  `AndroidSdkRoot` newtype, and their `#[cfg(test)]` tests into `android.rs`.

Resulting target: `checks/mod.rs` and `checks/android.rs` each comfortably under 500 lines.

### Details

- Move the Android functions **and their tests** verbatim — no logic changes. Adjust `use`/`super::`
  paths as needed.
- Preserve visibility: functions called from `toolchain/mod.rs` keep their existing `pub(crate)` /
  module visibility. The `AndroidSdkRoot` newtype stays `pub(super)`/`pub(crate)` as it was.
- Verify no public API surface changes: `fdemon_daemon::toolchain::run_preflight` and the re-exported
  report types are unaffected.

### Acceptance Criteria

1. `toolchain/checks/mod.rs` and `toolchain/checks/android.rs` are each under 500 lines.
2. No behavior change: the same component statuses are produced for the same inputs (existing tests,
   moved into `android.rs`, still pass unchanged).
3. `run_preflight` and all callers compile without changes to `toolchain/mod.rs`'s call sites
   (only `use` paths inside the `checks` module change).
4. Full quality gate green (fmt/check/test/clippy `-D warnings`).

### Testing

- This is a move-only refactor; the moved `#[cfg(test)]` tests are the regression guard. Run
  `cargo test -p fdemon-daemon` and confirm the **same test count** for the toolchain module
  (minus the duplicate removed in task 02).
- No new test logic required, but ensure the moved tests still reference the correct module paths.

### Notes

- Pure mechanical refactor — keep the diff to moves + path fixups so review is trivial.
- Do not "improve" the moved functions; any correctness change belongs in task 02.
- If a `git mv`-style move is cleaner for review, note in the completion summary which functions
  moved.
