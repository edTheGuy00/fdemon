# Bugfix Plan: Windows `flutter devices` Spawn Failure — Review Follow-up

## TL;DR

The Wave-1 fix for issues #32 and #34 (branch `fix/detect-windows-bat`) corrected the primary user-visible symptom (the `cmd /c` quote-stripping failure) and the latent UNC-prefix issue. Architecture, layer boundaries, MSRV bump, and the core `cmd /c` removal are all clean. However, the multi-agent code review (`workflow/reviews/bugs/windows-flutter-bat-spawn/`) surfaced four blocking concerns and several quality issues that must be addressed before the branch ships:

1. **One spawn path was missed (`emulators.rs`)** — three call sites still produce the old error format with no binary path and no Windows hint, creating an inconsistent diagnostic surface for users who hit failures during emulator discovery rather than device discovery.
2. **`info!` logging of `args` may leak dart-define secrets** — BUG.md Track 4 specified `debug!`; the implementation upgraded to `info!`. Combined with BUG.md Track 3's invitation to share log files with reporters, this is a real disclosure path.
3. **The shim-installer fix delivers less than promised** — chocolatey works only by coincidence, scoop/winget still fail because their shims do not live in `<root>/bin/`.
4. **Pre-existing clippy errors will land CI red on day one** — Rust 1.91's tightened lints fail across all 5 crates plus integration tests (~120 errors in 41 files). The new CI workflow runs `cargo clippy --workspace --all-targets -- -D warnings`, which means the first push fails on every platform.

This follow-up plan addresses all four blockers plus the minor cleanup items raised in `ACTION_ITEMS.md`. **Strategy decisions confirmed by the user:** (1) **clippy** — relax CI to `-W warnings` and file a separate workspace-wide cleanup bug rather than block this PR on a workspace refactor; (2) **shim-installer** — implement the real fix (Option B) so scoop and winget work without requiring users to set `[flutter] sdk_path`; (3) **scope** — ship Wave A + Wave B + Wave C in this single follow-up plan (no second polish PR).

See:
- `workflow/reviews/bugs/windows-flutter-bat-spawn/REVIEW.md` — consolidated multi-agent review
- `workflow/reviews/bugs/windows-flutter-bat-spawn/ACTION_ITEMS.md` — per-issue remediation steps
- `workflow/plans/bugs/windows-flutter-bat-spawn/BUG.md` — original Wave-1 plan and root cause analysis

---

## Bug Report

### Symptoms (post-Wave-1)

The Wave-1 fix is structurally correct and the primary symptom (#32, #34) is addressed. Reviewers identified the following residual issues:

| # | Issue | Severity | Source |
|---|-------|----------|--------|
| 1 | `emulators.rs` non-zero-exit and spawn-error branches lack the binary path, structured `error!` log, and `windows_hint()` that `devices.rs` now produces. | CRITICAL | `bug_fix_reviewer` |
| 2 | `process.rs:63-68` logs `args = ?args` at `info!`. dart-defines from `launch.toml` may contain secrets (API keys, OAuth client IDs, Sentry DSNs). BUG.md Track 4 specified `debug!`. | MAJOR (security) | `logic_reasoning_checker`, `security_reviewer` |
| 3 | Locator strategies 10/11 still require `<root>/bin/flutter.bat`. Scoop's shim is at `<root>/shims/`, winget's at `<root>/Links/` — both fail. Chocolatey works only because its shim happens to be at `<root>/bin/`. BUG.md claimed all three would work. | MAJOR | `logic_reasoning_checker` |
| 4 | Pre-existing clippy errors (~120 in 41 files spanning all 5 crates + integration tests) fail under `-D warnings`. The new CI workflow lands red. | MAJOR (CI) | Orchestrator final verification |
| 5 | 8 tests in `windows_tests.rs` violate the `test_<function>_<scenario>_<expected_result>` convention. | MINOR | `code_quality_inspector` |
| 6 | Spaces-in-path regression test passes against a fake `.bat` that ignores all arguments — proves spawn doesn't fail but doesn't verify args are passed correctly. | MINOR | `code_quality_inspector` |
| 7 | `windows_hint()` is appended on every non-zero `flutter devices` exit, regardless of cause. Could mislead users on adb/license/proxy failures. | MINOR | `logic_reasoning_checker` |
| 8 | `try_system_path()` is called twice (strategies 10 and 11), each invoking `which::which("flutter")` separately. | MINOR | `code_quality_inspector` |
| 9 | `InvalidInput` from dart-defines (post-CVE-2024-24576 escaper failure) produces an opaque "The parameter is incorrect" error with no Windows-specific guidance. | MINOR | `bug_fix_reviewer` |
| 10 | `FlutterExecutable::WindowsBatch` doc-comment claims it is a "metadata marker" but no production caller reads it as metadata — both arms behave identically. | MINOR | `logic_reasoning_checker` |
| 11 | `unwrap()` in test code lacks `.expect()` messages; `#[serial]` annotations lack rationale comments; `windows_tests.rs` uses non-portable `r"bin\cache\dart-sdk"` literals. | NITPICK | `code_quality_inspector` |
| 12 | ANSI escape sequences in flutter stderr embed verbatim into error strings. | NITPICK | `security_reviewer` |
| 13 | `.github/workflows/ci.yml` references `Swatinem/rust-cache@v2` and `actions/checkout@v4` by mutable tag rather than commit SHA. | NITPICK | `security_reviewer` |

### Environments

Same as the original bug — Windows 10/11 with Flutter on PATH. Reporters of #32 (`maxkabechani`) and #34 (`Far-Se`) were not yet asked to test the Wave-1 build.

---

## Root Cause Analysis

### Issue 1: `emulators.rs` was not in Task 04's scope

Task 04 ("Improve diagnostic logging") listed `devices.rs`, `process.rs`, and `version_probe.rs` in its "Files Modified (Write)" section. `emulators.rs` was overlooked despite calling `flutter.command()` the same way `devices.rs` did. This is a planning-time scope defect, not an implementation defect.

Three call sites in `crates/fdemon-daemon/src/emulators.rs` need the same diagnostic pattern that was applied to `devices.rs`:

- `run_flutter_emulators` (lines 126-160): both spawn-error and non-zero-exit branches.
- `run_flutter_emulator_launch` (lines 297-321): spawn-error branch only (the launch path intentionally does not treat non-zero as an error because emulators boot asynchronously).

Reuse the `windows_hint()` helper — currently `pub(crate)`-private to `devices.rs` (lines 240-254). Extracting it into a new `crates/fdemon-daemon/src/flutter_sdk/diagnostics.rs` module is the cleanest approach since `devices.rs` and `emulators.rs` are siblings.

### Issue 2: `args` logged at `info!` instead of `debug!`

Trace:
- `crates/fdemon-app/src/config/launch.rs:306-329` — `LaunchConfig::build_flutter_args()` constructs `--dart-define KEY=VALUE` pairs from user `launch.toml`.
- `crates/fdemon-app/src/config/vscode.rs:213-225` — VSCode `launch.json` import flows through the same builder.
- `crates/fdemon-daemon/src/process.rs:219` — `spawn_with_args()` accepts the args slice and forwards to `spawn_internal()`.
- `crates/fdemon-daemon/src/process.rs:63-68` — `info!(binary, args = ?args, cwd, "Spawning flutter session")`.

The original Wave-1 plan (`BUG.md` line 259, "Diagnostic logging") specified: *"log the full constructed command line (program + args + cwd) at `debug!`"*. The implementation upgraded the level to `info!`, which writes args (and any embedded secrets) to `%TEMP%\fdemon\fdemon-*.log`. BUG.md Track 3 explicitly invites users to share these log files for verification.

**Resolution:** Demote `args` to `debug!` per the original plan. Keep `binary` and `cwd` at `info!` — they are non-sensitive. (We rejected the redaction alternative because parsing all dart-define forms — `--dart-define KEY=VALUE`, `--dart-define-from-file=path`, etc. — is fragile and adds maintenance burden.)

### Issue 3: Shim-installer fix is incomplete (Option B selected)

The locator pipeline `which::which → resolve_sdk_root_from_binary → validate_sdk_path*` requires `<root>/bin/flutter.bat` to exist. Per-installer trace:

| Installer | Shim location | walk-up-2 | `<root>/bin/flutter.bat` exists? |
|-----------|---------------|-----------|----------------------------------|
| Real SDK | `<root>\bin\flutter.bat` | `<root>` | ✅ Yes |
| Chocolatey | `C:\ProgramData\chocolatey\bin\flutter.bat` | `C:\ProgramData\chocolatey` | ✅ Yes (same shim, by coincidence) |
| Scoop | `C:\Users\<u>\scoop\shims\flutter.bat` | `C:\Users\<u>\scoop` | ❌ No (uses `shims/`) |
| Winget | `%LOCALAPPDATA%\Microsoft\WinGet\Links\flutter.bat` | `...\WinGet` | ❌ No (uses `Links/`) |

**Option B (selected):** When `which::which("flutter")` succeeds but the inferred SDK root fails both `validate_sdk_path` and `validate_sdk_path_lenient`, fall back to a "binary-only" resolution: construct a `FlutterSdk` whose `executable` is the `which` result, `root` is `which_result.parent().unwrap_or(which_result)`, `source = SdkSource::PathInferred`, `version = "unknown"`, `channel = None`. The executable path alone is sufficient to spawn flutter — the SDK root is only needed for VERSION/channel metadata, which are non-essential.

This becomes a new "Strategy 12" or — equivalently — an additional fallback inside Strategy 11. We pick the first form (a separate strategy) because it is easier to reason about and easier to test.

### Issue 4: Pre-existing clippy errors block CI

Rust 1.91 tightened several lints:

- `field_reassign_with_default` (48 instances)
- `bool_assert_comparison` — e.g. `assert_eq!(x, true)` (16)
- `unused_mut`, `unused_variable` (24 combined)
- `type_complexity` (7)
- `manual_range_contains` (5)
- `assert!(true)` will-be-optimized-out (5)
- Misc (`useless_vec`, `while_let`, `clone-on-copy`, etc.) — ~10

These span 41 files across `fdemon-core`, `fdemon-daemon`, `fdemon-app`, `fdemon-tui`, `fdemon-dap`, and integration tests. Verified pre-existing against base commit `a455e4f`.

**Strategy A (selected):** Relax the CI clippy step from `-D warnings` to `-W warnings` (or omit `--all-targets`) so this PR can ship without being blocked on a workspace-wide refactor. File a dedicated bug `workflow/plans/bugs/clippy-rust-191-cleanup/` to track the cleanup as separate work. Once that bug ships, restore `-D warnings` to the CI clippy step.

This decoupling is correct because (a) the clippy errors pre-date the Windows fix and were not introduced by it, (b) blocking the user-visible Windows fix on a 100+ file mechanical refactor would harm reporters #32/#34, and (c) the cleanup is a discrete, parallelizable workstream that benefits from its own focused planning.

### Issues 5-13: Quality polish

These are independent of the four blockers. They split into:
- Test cleanup (5, 6, 11) — all in `windows_tests.rs`.
- Diagnostic surface polish (7, 8, 9, 12) — touches `devices.rs`, `locator.rs`, `process.rs`.
- Doc/comment honesty (10) — `types.rs` only.
- CI hardening (13) — `.github/workflows/ci.yml` and sibling workflow files.

---

## Suspect / Affected Code Locations

| File | Lines | Why |
|------|-------|-----|
| `crates/fdemon-daemon/src/emulators.rs` | 126-160, 297-321 | Three call sites need the diagnostic pattern from `devices.rs` |
| `crates/fdemon-daemon/src/devices.rs` | 213-254 | Hosts `windows_hint()` (to be moved); needs stderr-content-gated hint |
| `crates/fdemon-daemon/src/flutter_sdk/diagnostics.rs` | NEW | Shared `windows_hint()` (and possibly stderr-content predicates) |
| `crates/fdemon-daemon/src/flutter_sdk/locator.rs` | 184-219, 296-331 | Strategy 11 needs binary-only fallback; `try_system_path()` should be called once |
| `crates/fdemon-daemon/src/flutter_sdk/types.rs` | 54-66 | Honest `WindowsBatch` doc-comment |
| `crates/fdemon-daemon/src/flutter_sdk/windows_tests.rs` | All | Renames, strengthen spaces test, add scoop/winget tests, `.expect()`, `#[serial]` comments, portable joins |
| `crates/fdemon-daemon/src/process.rs` | 63-68, 80-88 | Demote args log to `debug!`; add `InvalidInput` arm |
| `.github/workflows/ci.yml` | 38-51 | Relax clippy gate; pin actions to SHAs |
| `workflow/plans/bugs/clippy-rust-191-cleanup/BUG.md` | NEW | Separate cleanup bug |
| `docs/ARCHITECTURE.md` | (`flutter_sdk/` section) | Reflect new `diagnostics.rs` module + new strategy 12 |

---

## Proposed Fix

### Strategy

Address the four blockers (Issues 1-4) and ship the user-facing improvement, then sweep the quality polish (Issues 5-13) as parallel sub-tasks where they don't conflict with the blockers' file modifications. Carry the shim-installer real fix (Issue 3) as a meaningful Locator change but keep its surface area narrow — one new strategy, one new pair of Windows tests.

### High-level changes

1. Extract `windows_hint()` from `devices.rs` into a new `crates/fdemon-daemon/src/flutter_sdk/diagnostics.rs`, then apply the diagnostic pattern from `devices.rs` to all three spawn paths in `emulators.rs`.
2. Demote `args = ?args` from `info!` to `debug!` in `process.rs:63-68`.
3. Relax `.github/workflows/ci.yml`'s clippy step (drop `-D warnings`) and scaffold a dedicated `workflow/plans/bugs/clippy-rust-191-cleanup/BUG.md` for the workspace-wide cleanup.
4. Add a binary-only fallback (Strategy 12) to the locator that handles scoop/winget/chocolatey shim layouts uniformly. Add Windows-only tests for both shim layouts.
5. Cleanup `windows_tests.rs` — rename to convention, strengthen the spaces-test, replace `unwrap()` with `.expect()`, comment `#[serial]` rationale, use portable `Path::join`. Add the new shim-layout tests delivered by Task 04 here.
6. Diagnostic surface polish — gate `windows_hint()` on stderr content (in the new `diagnostics.rs` module so both `devices.rs` and `emulators.rs` benefit); cache `try_system_path()` result across strategies 10/11; add `InvalidInput` arm in `process.rs`; strip ANSI from stderr; add doc note about explicit `[flutter] sdk_path`.
7. Honest `WindowsBatch` doc-comment in `types.rs` — remove the "metadata marker" claim that no production code currently reads.
8. Pin GitHub Actions in workflow files to commit SHAs.
9. Update `docs/ARCHITECTURE.md` to mention the new `diagnostics.rs` module, the new Strategy 12, and the diagnostics-gating change.

### Why these tradeoffs

- **`diagnostics.rs` as a new module** vs duplicating `windows_hint()` in `emulators.rs`: the helper will gain stderr-content-gating logic in Task 06, and that logic should be shared. Sharing now avoids a future deduplication step.
- **Real shim fix (Option B)** vs docs-only acknowledgment (Option A): user explicitly chose Option B. The cost is ~3h and two new tests; the benefit is that scoop and winget — the two most popular Windows package managers among Flutter developers per recent community surveys — work out of the box.
- **Relaxing CI clippy** vs full workspace cleanup in this PR: the cleanup is discrete and parallelizable but not Windows-bug-related. Keeping it separate keeps the PR scoped to the user-visible bug while not letting the existing clippy debt rot further (the new bug-plan is its commitment).

---

## Validation Strategy

### Track 1 — Unit tests

- Existing `windows_tests.rs` tests (8) re-run after rename and strengthening.
- New tests for scoop layout (`<temp>\scoop\shims\flutter.bat`) and winget layout (`<temp>\<root>\Links\flutter.bat`) under `#[cfg(target_os = "windows")]`.
- New `emulators.rs` unit test mirroring `test_run_flutter_devices_error_includes_binary_path` from `devices.rs`.

### Track 2 — Windows GitHub Actions runner

`.github/workflows/ci.yml` is already in place (Wave-1). After clippy is relaxed, the matrix should pass green on all three platforms.

### Track 3 — User-driven smoke test

Comment on issues #32 and #34 with a Windows test build (CI artifact from this branch) once the follow-up merges. Specifically ask reporters to:
- Confirm `flutter devices` no longer fails at startup.
- Confirm `flutter emulators` errors include the binary path on failure.
- Confirm dart-define values do not appear in the temp log file at info-level (they should only appear at debug).
- If they use scoop or winget, confirm fdemon now starts without setting `[flutter] sdk_path` manually.

---

## Risks / Open Questions

1. **Strategy 12 binary-only fallback may resolve a wrong Flutter executable.** If `which::which("flutter")` finds a non-Flutter `flutter` binary (e.g. a script named `flutter` on PATH that does something else), the fallback would happily use it. Mitigation: the fallback uses the `which` result regardless, but Strategy 12 only runs if the validate-strict and validate-lenient paths both reject the inferred root, so a real Flutter SDK never reaches Strategy 12. Misuse can still occur with truly stand-alone binaries (rare).
2. **Demoting args to `debug!` reduces diagnostic value for non-secret cases.** A user debugging a flag-parsing issue will need `RUST_LOG=fdemon_daemon::process=debug` (or similar) to see args. Acceptable — secrets matter more than convenience.
3. **Filing a separate clippy-cleanup bug delays the workspace-wide cleanup.** Mitigation: the new bug-plan is created in this PR so it cannot be forgotten.
4. **Pinning GitHub Actions to commit SHAs increases maintenance burden** (need to bump SHAs when actions release new versions). Mitigation: Renovate or Dependabot can automate the bumps. Acceptable.
5. **Doc updates depend on multiple implementation tasks landing in the right order.** Mitigation: the doc_maintainer task lists explicit dependencies in TASKS.md.

---

## Tasks

See `TASKS.md` for the full breakdown with overlap analysis. Eight implementor tasks + one doc_maintainer task across three waves.
