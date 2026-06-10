# Fix 01: Windows-aware JDK marker + sdkmanager error surfacing + pre-run java validation

**Status:** Not Started
**Agent:** implementor
**Complexity:** medium
**Depends On:** —
**Estimated Hours:** 2–3

## Objective

Three related fixes from the verified diagnosis in [../BUG.md](../BUG.md). All changes are in
`fdemon-daemon`; dev host is Linux, so every fix must be unit-testable cross-platform (extract
pure helpers / parameterize file names rather than relying on `#[cfg(windows)]`-only tests —
follow the existing patterns in `jdk.rs` and `android_install.rs` tests).

## Required Changes

### 1. `jdk.rs` — platform-aware javac marker in `java_home_from_which()`

At ~line 248 the marker check is
`jdk_home.join("release").is_file() || jdk_home.join("bin").join("javac").exists()`.
Make the javac name platform-aware exactly like `validate_jdk_home` already does
(`javac.exe` under `#[cfg(windows)]`, `javac` otherwise — see jdk.rs:93-98).
Prefer extracting a small pure helper (e.g. `fn has_jdk_markers(home: &Path, javac_name: &str) -> bool`)
so a Linux test can exercise BOTH names with a tempdir fixture. Update the nearby tracing message
(~line 254) if it hardcodes `bin/javac`. Also update the doc comment at ~line 212.

### 2. `android_install.rs` — carry the streamed log tail in failure errors

- Licenses run (~lines 371-389): `log_lines: Vec<String>` is already collected in scope. On
  `!status.success()`, replace "see log above for details" with the tail of the output, e.g.
  `"sdkmanager --licenses exited with {status}; last output: {tail}"` where `tail` is the last
  ~10 non-empty lines joined with `" | "` (cap total length to something sane, e.g. ~800 chars,
  to keep `WizardStepFailed.reason` readable). Extract the tail-formatting into a pure helper
  (e.g. `fn output_tail(lines: &[String], max_lines: usize, max_chars: usize) -> String`) with
  unit tests (empty, short, long, blank-line filtering, truncation).
- Package-install run (look for the `sdkmanager_packages` / package install invocation later in
  `install_android`, error arm ~lines 419-435): collect log lines the same way if not already
  collected, and apply the same helper to its failure message.

### 3. `android_install.rs` — validate the resolved JDK before running sdkmanager

After `build_sdkmanager_env(..)` succeeds (~line 358) and before the licenses run, execute
`<jdk_home>/bin/java[.exe] -version` (platform-aware name, same pattern as jdk.rs) via the
existing `run_streaming` with the SAME env pairs and the sdk_root cwd. On spawn failure or
non-zero exit, return a clear error such as:
`"resolved JDK at '<jdk_home>' cannot execute java ('<java_path> -version' failed: <detail>); set [toolchain] jdk_path in .fdemon/config.toml to a valid JDK 17+ home"`.

Implementation notes:
- `build_sdkmanager_env` currently resolves the JDK home internally; either change it to return
  `(Vec<(String,String)>, PathBuf /* jdk_home */)` and update its call sites/tests, or add a
  sibling accessor — pick whichever keeps the existing unit tests intact with minimal churn.
- Emit an `InstallEvent::Phase("Validating JDK")` (or a Log line) before the check so the wizard
  shows progress.
- Testing: do NOT spawn a real `java` in unit tests. Extract the java-exe path construction as a
  pure function (testable for both `java` / `java.exe` names) and test the error-message
  construction; the spawn itself reuses already-tested `run_streaming`. If the file has an
  existing pattern for fake-executable tests (shell-script stubs in a tempdir), a happy-path test
  with a stub `java` script is welcome on Unix (`#[cfg(unix)]`), but optional.

## Acceptance Criteria

- [ ] `java_home_from_which` accepts a JDK home whose `bin` contains `javac.exe` (Windows name)
      when configured with that marker name, and still accepts `javac`/`release`-file homes —
      proven by cross-platform unit tests of the extracted helper.
- [ ] A failed `sdkmanager --licenses` (and package-install) error message contains the tail of
      the streamed output instead of "see log above"; `output_tail` helper unit-tested.
- [ ] An invalid resolved JDK home produces the clear "cannot execute java" error naming the path
      BEFORE sdkmanager runs; java-exe path construction unit-tested for both platform names.
- [ ] No behavior change on the happy path (Linux licenses flow unchanged; existing
      `android_install.rs` + `jdk.rs` tests pass, updated only where messages changed).
- [ ] Quality gate: `cargo fmt --all -- --check && cargo check --workspace --all-targets &&
      cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`.

## Files

**Write:** `crates/fdemon-daemon/src/toolchain/jdk.rs`, `crates/fdemon-daemon/src/toolchain/android_install.rs`
**Read:** `crates/fdemon-daemon/src/toolchain/process_stream.rs` (run_streaming signature), `crates/fdemon-daemon/src/toolchain/mod.rs` (re-exports, if helper visibility changes)

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-platforms-submenu

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/toolchain/jdk.rs` | Extracted `has_jdk_markers(home, javac_name)` pure helper; updated `java_home_from_which` to use platform-aware javac name (`javac.exe` on Windows, `javac` on POSIX); updated doc comment on `java_home_from_which`; added 6 cross-platform unit tests for `has_jdk_markers` (both names, release-file acceptance, empty dir rejection, no cross-name contamination). |
| `crates/fdemon-daemon/src/toolchain/android_install.rs` | Added `output_tail(lines, max_lines, max_chars)` helper with blank-line filtering, `" | "` join, char-cap with ellipsis suffix; applied to `--licenses` and package-install failure paths; added `java_exe_path(jdk_home)` pure helper; changed `build_sdkmanager_env` return type to `(Vec<(String,String)>, PathBuf)` (tuple includes resolved `jdk_home`); added pre-run JDK validation (`java -version` via `run_streaming` with same env, emits `InstallEvent::Phase("Validating JDK")`); added `OUTPUT_TAIL_MAX_LINES` and `OUTPUT_TAIL_MAX_CHARS` named constants; updated all `build_sdkmanager_env` call sites and existing tests to destructure the tuple; added 8 `output_tail` unit tests + 3 `java_exe_path` unit tests. |

### Notable Decisions/Tradeoffs

1. **`build_sdkmanager_env` return-type change vs sibling accessor**: Returning `(env_pairs, jdk_home)` as a tuple avoids a duplicate resolution call and keeps the API honest — callers already needed both values. The alternative (a separate `resolve_jdk_home_for_sdkmanager` sibling) would repeat the resolution logic or add indirection. Existing tests updated with one-line destructure change each.

2. **Pre-validation via `run_streaming` (not `run_streaming_with_input`)**: `java -version` doesn't need stdin; using the simpler `run_streaming` is correct. The call uses the same env pairs as sdkmanager so any PATH or JAVA_HOME issue is caught in the exact same environment the bat script would see.

3. **`env_refs_owned` naming**: The temporary binding that was introduced to pass `&env_refs` to `run_streaming` and then again to `run_streaming_with_input` was renamed `env_refs_owned` (before its shadow `env_refs`) to avoid a confusing shadowed binding while keeping the original structure familiar. `rustfmt` reformatted the call-site line.

4. **`has_jdk_markers` visibility `pub(crate)`**: Needed only within `fdemon-daemon`; crate-pub is sufficient and avoids polluting the public API of `jdk.rs`.

### Testing Performed

- `cargo fmt --all -- --check` — Passed
- `cargo check --workspace --all-targets` — Passed
- `cargo test --workspace` — Passed (1251 daemon unit tests; 0 failures)
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed

### Risks/Limitations

1. **`java -version` adds a spawn before every sdkmanager run on the happy path**: On a valid JDK this is a ~50 ms cost (JVM start, print version, exit). This is acceptable for a wizard step where the user is already waiting for a multi-second download+install. The validation is only reached after `check_sdkmanager_guard` has already confirmed cmdline-tools is installed, so it does not affect non-Android wizard flows.
