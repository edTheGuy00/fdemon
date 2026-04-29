# Review: Windows `flutter devices` Spawn Failure

**Bug:** [`workflow/plans/bugs/windows-flutter-bat-spawn/BUG.md`](../../../plans/bugs/windows-flutter-bat-spawn/BUG.md)
**Branch:** `fix/detect-windows-bat`
**Diff Base:** `a455e4f` → `cef20e6`
**Reviewed:** 2026-04-26
**Issues:** GitHub #32, #34

---

## Verdict: ⚠️ NEEDS WORK

The fix correctly addresses the primary user-visible symptom (the `cmd /c` quote-stripping failure for paths with whitespace) and the latent UNC-prefix issue. The architecture is clean, dependency placement is correct, and the security posture is preserved. However, three reviewers independently surfaced concerns that block clean approval:

1. **One spawn path (`emulators.rs`) was missed** — it still produces the old error format with no binary path and no Windows hint. Users discovering this on Windows after the fix ships will see inconsistent diagnostics.
2. **`info!` logging of `args` (which include dart-defines from `launch.toml`) creates a real secret-leak risk** — the original BUG.md plan specified `debug!` for spawn args precisely to avoid this; the implementation upgraded the level. This matters now because BUG.md Track 3 explicitly invites users to share log files for verification.
3. **The shim-installer fix delivers less than BUG.md claimed** — chocolatey works only by coincidence (its shim happens to live at `<root>/bin/`); scoop and winget still fail. BUG.md's "having a working executable path is sufficient" guidance was not implemented.

None of these are defects in what was *built*; the architecture, layer boundaries, dependency consumption, MSRV bump, CI workflow, and the core `cmd /c` removal are all correct. The concerns are about completeness, log hygiene, and stated-versus-delivered scope.

---

## Source Reviews

| Agent | Verdict |
|-------|---------|
| `bug_fix_reviewer` | ⚠️ CONCERNS |
| `architecture_enforcer` | ✅ PASS |
| `code_quality_inspector` | ⚠️ APPROVED WITH RESERVATIONS |
| `logic_reasoning_checker` | ⚠️ WARNING / CONCERNS |
| `security_reviewer` | ✅ PASS (4 LOW notes) |

Per the skill's verdict matrix: multiple agents flagged ⚠️ CONCERNS → **NEEDS WORK**.

---

## Critical Issues

### 1. `emulators.rs` still produces old-style errors (no binary path, no Windows hint)
- **Source:** `bug_fix_reviewer`
- **File:** `crates/fdemon-daemon/src/emulators.rs:135-141, 151-157, 315-320`
- **Problem:** Three call sites in `emulators.rs` go through `flutter.command()` and produce errors that look like:
  ```
  flutter emulators failed with exit code Some(1): <stderr>
  ```
  Compare to the updated `devices.rs:226-232`:
  ```
  flutter devices failed (binary: <path>, exit code Some(1)): <stderr>
  Hint: If your Flutter is installed via a package manager...
  ```
  The non-zero-exit branch in `emulators.rs:151-157` does not log structured fields at `error!`, does not include the binary path, and does not append `windows_hint()`. The spawn-error branch at `emulators.rs:135-141` does not include the binary path either.
- **Why it matters:** A Windows user who triggers emulator discovery (e.g., before any device is online) will hit the same class of failure that #32/#34 reported, but with strictly less actionable diagnostic text than `flutter devices` produces. This is the exact inconsistency the diagnostic-logging task (04) was meant to prevent.
- **Required action:** Apply the same pattern from `devices.rs:219-253` to `emulators.rs:135-157` (and the same pattern to `emulators.rs:315-320`). Promote the non-zero-exit log to `error!` with structured fields, embed the binary path in the error string, and append `windows_hint()`.
- **Note on scope:** Task 04's "Files Modified (Write)" list did not include `emulators.rs`. This is a scoping defect in the plan. The plan should have either listed `emulators.rs` or scheduled a follow-up task; instead it shipped an inconsistent diagnostic surface.

---

## Major Issues

### 2. Spawn args logged at `info!` may leak dart-define secrets
- **Source:** `logic_reasoning_checker` (W1), `security_reviewer` (LOW)
- **File:** `crates/fdemon-daemon/src/process.rs:63-68`
- **Problem:** The structured `info!` log line emits `args = ?args, cwd = ..., binary = ...`. The args slice contains dart-define key=value pairs sourced from `launch.toml`. dart-defines are commonly used by Flutter teams to inject API keys, OAuth client IDs, Sentry DSNs, and Firebase config at build time — these are secrets in many real projects.
- **Why it matters:** BUG.md Track 4 explicitly specified `debug!` for spawn args ("log the full constructed command line (program + args + cwd) at `debug!`"). The implementation upgraded this to `info!`, which contradicts the plan. Combined with BUG.md Track 3's explicit invitation to reporters to share `%TEMP%\fdemon\fdemon-*.log` files for verification, this creates a real disclosure path.
- **Required action:** Demote `args = ?args` from `info!` to `debug!` per the original plan. The `binary` and `cwd` fields can stay at `info!`. Alternative: redact dart-define values at the log site (log the keys but not values) and keep `info!`.

### 3. Shim-installer fix delivers less than BUG.md claimed
- **Source:** `logic_reasoning_checker` (C2, C3)
- **File:** `crates/fdemon-daemon/src/flutter_sdk/locator.rs:185-219, 296-331`
- **Problem:** BUG.md (line 199) states: "This fixes shim-style installs because we no longer try to derive an SDK root from the shim location — for strategy 10/11, having a working executable path is sufficient." The implementation does NOT honor this — it still calls `resolve_sdk_root_from_binary` (walk-up-2) and requires `<root>/bin/flutter.bat` to exist via `validate_sdk_path` / `validate_sdk_path_lenient`. Trace per installer:
  - **Chocolatey** (`C:\ProgramData\chocolatey\bin\flutter.bat`): walks up to `C:\ProgramData\chocolatey`, then validates `C:\ProgramData\chocolatey\bin\flutter.bat` — happens to be the same shim, so it works *by coincidence*.
  - **Scoop** (`C:\Users\<u>\scoop\shims\flutter.bat`): walks up to `C:\Users\<u>\scoop`, then validates `C:\Users\<u>\scoop\bin\flutter.bat` — does not exist (scoop uses `shims/`, not `bin/`). Fails.
  - **Winget** (`%LOCALAPPDATA%\Microsoft\WinGet\Links\flutter.bat`): same failure mode.
- **Why it matters:** Two reasonable user reactions: "I installed Flutter via scoop and it still doesn't work" or "I installed via chocolatey and it works, sometimes." The current state is non-deterministic and the BUG.md / TASKS.md / docs all imply the fix handles all three.
- **Required action:** Either:
  - **(A)** Update BUG.md and `docs/ARCHITECTURE.md` to acknowledge that only chocolatey is incidentally fixed and scoop/winget still require the explicit `[flutter] sdk_path` override (which the new `windows_hint()` already directs users to).
  - **(B)** Implement the BUG.md-as-stated behavior: when `which::which("flutter")` succeeds but neither `validate_sdk_path` nor `validate_sdk_path_lenient` accepts the inferred root, return a `FlutterSdk` with `executable = WindowsBatch(which_result)`, `root = which_result.parent()`, `source = SdkSource::PathInferred`, `version = "unknown"`.
- **Recommendation:** (A) is cheaper for this PR; (B) is the right long-term fix and could be a follow-up task. Pick one and reflect it consistently in BUG.md and the user-facing hint.

### 4. `cargo clippy --workspace -- -D warnings` fails on the merged tree (pre-existing, but newly relevant)
- **Source:** Final orchestration verification (orchestrator)
- **Files:** `crates/fdemon-daemon/src/vm_service/extensions/...` (10 errors), `crates/fdemon-dap/src/adapter/tests/...` (35 errors)
- **Problem:** Rust 1.91's tightened clippy lints (`bool_assert_comparison`, `type_complexity`) flag pre-existing code in unrelated crates. Verified against base commit `a455e4f` — the same errors existed before this work.
- **Why it matters:** The new CI workflow runs `cargo clippy --workspace --all-targets -- -D warnings` on three platforms. The first push to a branch with `.github/workflows/ci.yml` will fail clippy on every platform until these are fixed. CI will be broken on day one.
- **Required action:** Fix the pre-existing clippy errors in `fdemon-daemon/src/vm_service/extensions/` and `fdemon-dap/src/adapter/tests/` BEFORE merging this branch — otherwise the CI lands in a permanently-failing state. This is out of scope for "the bug fix" but in scope for "shipping CI that works." Either fix them on this branch as a prerequisite commit, or temporarily relax `-D warnings` to `--no-deps` or omit it from the clippy step.

---

## Minor Issues

### 5. Test naming in `windows_tests.rs` violates the `test_<function>_<scenario>_<expected_result>` convention
- **Source:** `code_quality_inspector`
- **File:** `crates/fdemon-daemon/src/flutter_sdk/windows_tests.rs` (all 8 tests)
- **Problem:** Functions are named `validate_sdk_path_returns_windows_batch_variant`, `windows_batch_command_invokes_path_directly`, etc. — none have the `test_` prefix required by `docs/CODE_STANDARDS.md`. Cross-platform tests in `locator.rs` and `types.rs` use the correct prefix.
- **Suggested action:** Rename to e.g. `test_validate_sdk_path_windows_returns_windows_batch_variant`.

### 6. The spaces-in-path regression test is weaker than it looks
- **Source:** `code_quality_inspector`, `architecture_enforcer` (suggestion)
- **File:** `crates/fdemon-daemon/src/flutter_sdk/windows_tests.rs:99-116`
- **Problem:** `windows_batch_command_works_with_path_containing_spaces` runs `exe.command().arg("devices").output()` against a fake `.bat` that always exits 0 regardless of args. The test asserts `output.status.success()`, which proves "spawn did not fail" — that *is* the regression scenario for #32/#34, since the original bug failed at the spawn level. But it does not prove that the argument was passed correctly through any path-quote-stripping edge cases.
- **Suggested action:** Have the fake `.bat` echo `%*` to stdout and assert the test argument appears in the output. Cheap and meaningfully strengthens the test.

### 7. `windows_hint()` is appended on every non-zero exit, regardless of cause
- **Source:** `logic_reasoning_checker` (W2)
- **File:** `crates/fdemon-daemon/src/devices.rs:226-232, 245-249`
- **Problem:** The hint targets path-resolution failures specifically, but `flutter devices` exits non-zero for many unrelated reasons (adb crashed, license not accepted, network proxy). The hint will mislead some users.
- **Suggested action:** Gate the append on a stderr substring match (e.g. `stderr.contains("cannot find the path") || stderr.contains("not recognized as an internal")`). The hint already has good softening language ("If your Flutter is installed via..."), but a content gate would prevent it from showing on unrelated failures entirely.

### 8. `try_system_path()` invoked twice in strategies 10 and 11
- **Source:** `code_quality_inspector`, `architecture_enforcer` (deferred)
- **File:** `crates/fdemon-daemon/src/flutter_sdk/locator.rs:185, 196`
- **Problem:** Each invocation now performs a `which::which("flutter")` PATH walk. Strategies 10 and 11 share the same `which` result; only the validation strictness differs. Two PATH walks instead of one.
- **Note:** BUG.md "Risks/Open Questions #8" explicitly noted this redundancy and chose to defer cleanup to a follow-up.
- **Suggested action:** Cache the result in a local variable for this PR (one-line change), or accept the deferral and file a follow-up issue.

### 9. `InvalidInput` from dart-defines produces a generic, opaque error
- **Source:** `bug_fix_reviewer`
- **File:** `crates/fdemon-daemon/src/process.rs:80-88`
- **Problem:** Post-CVE-2024-24576, Rust returns `Err(ErrorKind::InvalidInput)` from `Command::spawn()` if it cannot safely escape `.bat` arguments. A user with a dart-define value containing `%`, `^`, `&`, `|`, `<`, `>`, or unmatched `"` will see something like `"The parameter is incorrect. (binary: C:\flutter\bin\flutter.bat)"` with no actionable explanation.
- **Note:** BUG.md "Risks/Open Questions #7" and TASKS.md notes both flag this as out-of-scope but called for a clear error.
- **Suggested action:** Add an `e.kind() == std::io::ErrorKind::InvalidInput` arm before the catch-all in `process.rs:80-88`, emitting a Windows-targeted message naming dart-define escaping as the likely cause. Two-line change.

### 10. `WindowsBatch` enum variant is operationally dead
- **Source:** `logic_reasoning_checker` (C1)
- **File:** `crates/fdemon-daemon/src/flutter_sdk/types.rs:54-93`
- **Problem:** Both `Direct(p)` and `WindowsBatch(p)` arms of `path()` and `command()` are operationally identical. No production caller pattern-matches on the variant. The variant carries strictly less information than the path's extension (`.bat`/`.cmd`).
- **Note:** BUG.md "Risks/Open Questions #3" explicitly chose to keep the enum to avoid API churn. This was a conscious decision, not an oversight. But the doc-comment justification ("metadata marker so callers can tell that the underlying executable is a batch file") is currently fictional — no caller reads it as metadata.
- **Suggested action:** Either (a) accept the enum-as-future-marker rationale and trust the doc-comment, or (b) reduce the doc-comment to honest text ("placeholder kept for backward compat; both variants behave identically — extension-based detection is preferred"). Defer collapsing the enum to a separate refactor.

---

## Nitpicks

### 11. `unwrap()` in test code should be `.expect("...")` with a message
- **Source:** `code_quality_inspector`
- **File:** `windows_tests.rs:87-88, 107-108`

### 12. `#[serial]` annotations lack a comment explaining why they are required
- **Source:** `code_quality_inspector`
- **File:** `windows_tests.rs:31-50`
- **Note:** The `serial_test` crate is needed because the tests mutate process-wide `PATH`. Cross-platform tests in `locator.rs` add a comment explaining this. The Windows tests do not.

### 13. `create_fake_sdk` uses Windows-style `r"bin\cache\dart-sdk"` literals
- **Source:** `code_quality_inspector`, `architecture_enforcer`
- **File:** `windows_tests.rs:21-22`
- **Note:** Works correctly on Windows (the only platform that compiles this file), but `root.join("bin").join("cache").join("dart-sdk")` would be portable.

### 14. ANSI escape sequences in `flutter` stderr are passed verbatim into error strings
- **Source:** `security_reviewer` (LOW)
- **File:** `crates/fdemon-daemon/src/devices.rs:226-232`
- **Note:** Ratatui doesn't interpret raw ANSI in rendered text, so this is not a terminal-injection vector. Worth stripping for log readability.

### 15. CI references `Swatinem/rust-cache@v2` and other actions by mutable tag
- **Source:** `security_reviewer` (LOW)
- **File:** `.github/workflows/ci.yml`
- **Note:** Pinning to commit SHAs would harden against tag mutation. Standard, low-priority hardening.

### 16. `try_system_path()` doc comment could note that PATH-trust users in security-sensitive environments should pin via `[flutter] sdk_path`
- **Source:** `security_reviewer` (LOW)
- **File:** `crates/fdemon-daemon/src/flutter_sdk/locator.rs`

---

## Documentation Freshness

- ✅ `docs/ARCHITECTURE.md` was updated (task 07) and accurately reflects `which`/`dunce` usage and `FlutterExecutable` semantics.
- ✅ `docs/DEVELOPMENT.md` was updated (task 07) — MSRV 1.77.2, CI section, Windows Common Issues entry.
- ⚠️ If Major Issue #3 above is resolved by Option A (acknowledge limitation), `docs/ARCHITECTURE.md` should be updated again to reflect the actual scope of the shim fix (chocolatey-incidental, not scoop/winget).
- ✅ No new modules, error types, or coding patterns were introduced that require `docs/CODE_STANDARDS.md` updates.

---

## Verification

The orchestrator ran a final verification gate after Wave 3:

| Check | Result |
|-------|--------|
| `cargo fmt --all -- --check` | ✅ PASS (after follow-up rustfmt commit `cef20e6`) |
| `cargo check --workspace` | ✅ PASS |
| `cargo test -p fdemon-daemon --lib` | ✅ PASS (736 passed, 0 failed, 3 ignored) |
| `cargo clippy --workspace -- -D warnings` | ❌ FAIL — but pre-existing, verified against `a455e4f` (see Major Issue #4) |

---

## Recommendation

**Address Critical Issue #1, Major Issues #2/#3/#4 before merge.** The remaining Minors and Nitpicks are non-blocking and can ship in a follow-up.

The orchestrator's main-line fix (the `cmd /c` removal, `which`/`dunce` adoption, MSRV bump, CI workflow, and Windows test scaffold) is structurally sound and the architecture is clean. The blockers are all cleanup work that should have been part of the original task scope.

See `ACTION_ITEMS.md` for the prioritized fix list.
