# Action Items: Windows `flutter devices` Spawn Failure

**Review Date:** 2026-04-26
**Verdict:** ⚠️ NEEDS WORK
**Blocking Issues:** 4

---

## Critical Issues (Must Fix)

### 1. Apply diagnostic-error pattern to `emulators.rs`
- **Source:** `bug_fix_reviewer`
- **File:** `crates/fdemon-daemon/src/emulators.rs`
- **Lines:** `135-141` (spawn error), `151-157` (non-zero exit), `315-320` (launch spawn error)
- **Problem:** Three call sites still produce the old error format. They lack:
  - structured `error!` log with `binary`, `exit_code`, `stderr`, `stdout` fields
  - the resolved binary path in the user-facing error string
  - the `windows_hint()` append on Windows
- **Required Action:**
  - Mirror the pattern from `crates/fdemon-daemon/src/devices.rs:219-253` in `run_flutter_emulators` (`emulators.rs:126-160`)
  - Apply the same pattern to the spawn-error branch in `run_flutter_emulator_launch` (`emulators.rs:297-321`)
  - Use `stderr.trim()` consistently (matches `devices.rs` and avoids trailing whitespace in the message)
  - Note: `windows_hint()` is currently `pub(crate)`-private to `devices.rs`. Either move it to a shared location (e.g. `crates/fdemon-daemon/src/flutter_sdk/mod.rs` or a new `diagnostics.rs`) or duplicate it in `emulators.rs` with the same `#[cfg(target_os = "windows")]` gate.
- **Acceptance:** A unit test in `emulators.rs` (mirroring the new test in `devices.rs:668-680`) asserts that an error from a non-existent flutter path includes the resolved binary path. Both `run_flutter_emulators` and `run_flutter_emulator_launch` error strings include `(binary: <path>)` when the flutter spawn fails or exits non-zero.

---

## Major Issues (Should Fix)

### 2. Demote `args = ?args` from `info!` to `debug!`
- **Source:** `logic_reasoning_checker`, `security_reviewer`
- **File:** `crates/fdemon-daemon/src/process.rs:63-68`
- **Problem:** dart-define values from `launch.toml` may contain API keys, OAuth client IDs, Sentry DSNs, or Firebase config. Logging them at `info!` level writes them to log files in `%TEMP%\fdemon\fdemon-*.log`. BUG.md Track 3 explicitly invites users to share these log files for verification.
- **Required Action:** Change the structured log to:
  ```rust
  info!(
      binary = %flutter.path().display(),
      cwd = %project_path.display(),
      "Spawning flutter session"
  );
  debug!(
      binary = %flutter.path().display(),
      args = ?args,
      cwd = %project_path.display(),
      "Spawning flutter session (with args)"
  );
  ```
  Or, if a single statement is preferred, redact dart-defines:
  ```rust
  let redacted: Vec<String> = args.iter().map(|a| {
      if a.starts_with("--dart-define=") {
          if let Some(eq) = a[14..].find('=') {
              format!("--dart-define={}=<redacted>", &a[14..14+eq])
          } else { a.clone() }
      } else { a.clone() }
  }).collect();
  info!(binary = ..., args = ?redacted, cwd = ..., "Spawning flutter session");
  ```
- **Acceptance:**
  - `RUST_LOG=info` traces no longer contain dart-define values
  - `RUST_LOG=debug` traces continue to include them for active debugging
  - A test asserts the redaction (if redaction is the chosen approach)

### 3. Resolve the shim-installer scope discrepancy
- **Source:** `logic_reasoning_checker`
- **File:** `crates/fdemon-daemon/src/flutter_sdk/locator.rs` and `workflow/plans/bugs/windows-flutter-bat-spawn/BUG.md`
- **Problem:** BUG.md claims the fix handles shim-style installers (Chocolatey, scoop, winget). In practice only chocolatey works (incidentally — its shim happens to be in `<root>/bin/`). Scoop and winget fail because their shims live in `shims/` or `Links/`, not `bin/`.
- **Required Action — pick one:**
  - **Option A (cheaper):** Update BUG.md and `docs/ARCHITECTURE.md` to acknowledge that scoop/winget still require the explicit `[flutter] sdk_path` override. Update the in-binary `windows_hint()` to lean into this ("If you installed via scoop or winget, set..."). File a follow-up issue for full shim support.
  - **Option B (correct fix):** In `find_flutter_sdk` strategy 11, accept a `which::which` result whose inferred SDK root fails `validate_sdk_path_lenient`. Construct a `FlutterSdk` with `executable = WindowsBatch(which_result)`, `root = which_result.parent().unwrap_or(which_result)`, `source = SdkSource::PathInferred`, `version = "unknown"`. Add Windows-only tests covering the scoop and winget shim layouts.
- **Acceptance:**
  - Option A: BUG.md, `docs/ARCHITECTURE.md`, and `windows_hint()` all describe the same scope consistently.
  - Option B: New Windows-only tests pass for scoop layout (`<temp>\scoop\shims\flutter.bat`) and winget layout (`<temp>\Links\flutter.bat`).

### 4. Fix pre-existing clippy errors before CI lands
- **Source:** Final orchestration verification
- **Files:**
  - `crates/fdemon-daemon/src/vm_service/extensions/...` — 10 errors (mostly `bool_assert_comparison`, e.g. `crates/fdemon-daemon/src/vm_service/extensions/parsers.rs:340`)
  - `crates/fdemon-dap/src/adapter/tests/...` — 35 errors (`type_complexity`, e.g. `crates/fdemon-dap/src/adapter/tests/update_debug_options.rs:52`)
- **Problem:** The new CI workflow runs `cargo clippy --workspace --all-targets -- -D warnings` on three platforms. With Rust 1.91's tightened lints, the workspace fails clippy from day one — verified against base commit `a455e4f`. The first push lands CI in a permanently-red state.
- **Required Action — pick one:**
  - **Option A (recommended):** Fix the clippy errors as a prerequisite commit on this branch. They are mechanical:
    - `assert_eq!(x, true)` → `assert!(x)`; `assert_eq!(x, false)` → `assert!(!x)`
    - Extract `Arc<Mutex<Vec<(String, String, bool)>>>` etc. into `type` aliases at module scope
  - **Option B (workaround):** Drop `--all-targets` from the clippy step in `ci.yml`, or downgrade `-D warnings` to `-W warnings` for now. Less safe — it lets future regressions in test code slip through.
- **Acceptance:** `cargo clippy --workspace --all-targets -- -D warnings` exits 0 on macOS and Linux locally (Windows verified via CI).

---

## Minor Issues (Consider Fixing)

### 5. Rename tests in `windows_tests.rs` to match the project naming convention
- **File:** `crates/fdemon-daemon/src/flutter_sdk/windows_tests.rs`
- **Action:** Add `test_` prefix to all 8 functions and follow `test_<function>_<scenario>_<expected_result>` per `docs/CODE_STANDARDS.md`.

### 6. Strengthen `windows_batch_command_works_with_path_containing_spaces`
- **File:** `crates/fdemon-daemon/src/flutter_sdk/windows_tests.rs:99-116`
- **Action:** Modify the fake `.bat` to echo `%*`; assert the test argument appears in stdout.

### 7. Gate `windows_hint()` append on stderr content matching path-resolution errors
- **File:** `crates/fdemon-daemon/src/devices.rs:226-232, 245-249`
- **Action:** Only append the hint when stderr matches `cannot find the path` or `not recognized as an internal`. Avoids misleading users on unrelated `flutter devices` failures.

### 8. Cache `try_system_path()` result across strategies 10 and 11
- **File:** `crates/fdemon-daemon/src/flutter_sdk/locator.rs:185, 196`
- **Action:** Bind `which::which("flutter")` once into a local variable; use it for both strict and lenient validation.

### 9. Add `InvalidInput` spawn-error arm
- **File:** `crates/fdemon-daemon/src/process.rs:80-88`
- **Action:** Match `e.kind() == std::io::ErrorKind::InvalidInput` before the catch-all. On Windows, surface a message like:
  ```
  flutter spawn rejected an argument it could not safely escape (binary: <path>).
  This usually means a dart-define value contains characters cmd.exe cannot pass safely
  (% ^ & | < > unmatched "). Check launch.toml.
  ```

### 10. Honest doc comment on `FlutterExecutable::WindowsBatch`
- **File:** `crates/fdemon-daemon/src/flutter_sdk/types.rs:54-66`
- **Action:** The current "metadata marker" justification is fictional (no caller reads it as metadata). Either trust the future-use rationale and leave it, or reduce the doc to: "Placeholder for Windows `.bat` paths. Operationally identical to `Direct` — extension-based detection (`.bat`/`.cmd`) is the canonical signal."

---

## Nitpicks

- Replace `unwrap()` in `windows_tests.rs:87-88, 107-108` with `.expect("...")` and a message.
- Add comments to each `#[serial]` annotation in `windows_tests.rs:31-50` explaining the PATH-mutation rationale.
- Use `root.join("bin").join("cache").join("dart-sdk")` instead of `r"bin\cache\dart-sdk"` literals in `windows_tests.rs:21-22`.
- Strip ANSI escape sequences from flutter stderr before embedding in error strings (`devices.rs:226-232`).
- Pin GitHub Actions to commit SHAs in `.github/workflows/ci.yml`.
- Mention the explicit-`sdk_path` mitigation in the `try_system_path()` doc comment.

---

## Re-review Checklist

After addressing issues, the following must pass:

- [ ] `emulators.rs` non-zero-exit and spawn-error branches include the binary path and (on Windows) the hint
- [ ] `process.rs:63-68` no longer logs `args = ?args` at `info!`
- [ ] BUG.md and the `windows_hint()` text describe the same scope of shim-installer support
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` exits 0 on Linux + macOS
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `cargo test --workspace` passes
- [ ] CI workflow runs green on all three platforms (verified via the new `.github/workflows/ci.yml`)
- [ ] No new clippy violations introduced

After the four blocking issues are resolved, the remaining Minors and Nitpicks can ship as follow-ups in a separate PR.
