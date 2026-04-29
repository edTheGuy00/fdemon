# Bugfix Plan: CI 3-OS Portability Fixes (PR #38 follow-up)

## TL;DR

PR #38 (`fix/detect-windows-bat`, "Fix Windows flutter.bat spawn (#32, #34) + Rust 1.91 / MSRV cleanup") added the project's first 3-OS CI matrix (`.github/workflows/ci.yml`). The new matrix surfaces 10 **pre-existing** test failures that were never noticed because the project previously had no CI: 2 on Linux (`fdemon-daemon`), 1 on macOS (`fdemon-daemon`), 7 on Windows (`fdemon-app`). The failures are not regressions from this PR's intent — they are portability bugs in test code (and a few in production code) that the new CI exposes.

This bug also addresses 3 minor reviewer comments left by the Copilot reviewer on PR #38: an inaccurate workflow comment about toolchain pinning, a docs/CI command mismatch, and an incorrect `launch.toml` path in a Windows error message.

The 10 failures cluster into 6 root causes (tool gate, env-var leakage, IDE path separators, time arithmetic, HTTP server race) plus 3 reviewer-comment edits → planned as **8 disjoint-write-file tasks** that all run in parallel in a single wave. Total estimated effort: ~3.5 hours.

CI run referenced throughout: <https://github.com/edTheGuy00/fdemon/actions/runs/24998544748>.

---

## Bug Reports

### Bug 1: `tool_availability::tests::test_native_logs_available_ios_with_simctl` and `…_with_idevicesyslog` fail on Linux/Windows

**Symptom (Ubuntu, Windows):** Two tests in `crates/fdemon-daemon/src/tool_availability.rs` panic with `assertion failed: tools.native_logs_available("ios")`.

**Expected:** The tests should pass on the platforms where their assertions are meaningful (macOS only) and not run on platforms where the production code is intentionally absent.

**Root Cause Analysis:**
1. `ToolAvailability::native_logs_available(&self, platform)` (in `tool_availability.rs`) gates the `"ios"` match arm with `#[cfg(target_os = "macos")]`. On Linux and Windows the arm is absent and `"ios"` falls through to the `_ => false` catch-all.
2. The tests construct a `ToolAvailability` with the relevant iOS field set to `true` and assert that `native_logs_available("ios")` returns `true`. This assertion can only hold on macOS.
3. Neither test has `#[cfg(target_os = "macos")]`, so the tests run on every platform and fail wherever the match arm is absent.
4. The companion `test_native_logs_available_ios_no_tools` (asserts `false`) correctly passes everywhere because `false` is what the catch-all returns.

**Affected Files:**
- `crates/fdemon-daemon/src/tool_availability.rs:379-396` — both failing tests, both need a `#[cfg(target_os = "macos")]` attribute.

---

### Bug 2: `flutter_sdk::locator::tests::test_flutter_wrapper_detection` fails on macOS due to `FLUTTER_ROOT` leaking from the runner environment

**Symptom (macOS):** `assertion left == right failed; left: SdkSource::EnvironmentVariable, right: SdkSource::FlutterWrapper` at `crates/fdemon-daemon/src/flutter_sdk/locator.rs:664`.

**Expected:** The wrapper-detection strategy (Strategy 9) should win when a project has a `flutterw` script — even if the host environment has `FLUTTER_ROOT` set globally.

**Root Cause Analysis:**
1. `find_flutter_sdk` walks 12 strategies in order. Strategy 2 reads `std::env::var_os("FLUTTER_ROOT")` and returns immediately if it resolves to a valid SDK. Strategy 9 is wrapper detection.
2. GitHub's macOS runners pre-install Flutter and set `FLUTTER_ROOT` in the environment. Strategy 2 fires before Strategy 9.
3. Other priority-order tests in the same file (`test_fvm_modern_detection`, `test_flutter_root_env_beats_version_managers`, etc.) already use `#[serial_test::serial]` and `std::env::remove_var("FLUTTER_ROOT")` to scrub the environment. `test_flutter_wrapper_detection` was missed.
4. This is a test bug, not a production bug — the priority order is intentional, and on a developer machine without `FLUTTER_ROOT` the test passes.

**Affected Files:**
- `crates/fdemon-daemon/src/flutter_sdk/locator.rs:652-666` — add `#[serial]` and `std::env::remove_var("FLUTTER_ROOT")` to match the pattern used by sibling tests.

---

### Bug 3: Three Emacs IDE-config tests fail on Windows due to backslash separators in embedded Lisp paths

**Symptom (Windows):** Three tests in `crates/fdemon-app/src/ide_config/emacs.rs` fail because the production code embeds OS-native paths in the generated Elisp via `path.display().to_string()`. On Windows this yields backslashes; the tests assert forward-slash literals.

- `test_emacs_generate_embeds_absolute_path` (line 182)
- `test_emacs_merge_uses_absolute_path` (line 192)
- `test_emacs_merge_produces_absolute_path` (line 203)

**Expected:** The generated `dap-emacs.el` content should embed paths using forward slashes on every platform, since Emacs Lisp string literals treat `\` as an escape character (`"\f"` is form-feed, not the start of a path) and `load-file` accepts `/` natively on Windows.

**Root Cause Analysis:**
1. In `emacs.rs`, `generate_elisp` (called by both `generate` and `merge_config`) embeds `path.display().to_string()` directly in the Elisp output.
2. On Windows, `Path::display()` emits backslashes — both as path separators and (in the embedded Lisp string) as accidental escape sequences.
3. The convention in Lisp / Emacs config files is forward slashes, even on Windows. Production code is wrong.
4. Tests 4 and 5 assert the literal `"/my/flutter/app/.fdemon/dap-emacs.el"` and will pass once production normalises `\` to `/`. Test 6 constructs `expected_path = tempdir.path().join(".fdemon/dap-emacs.el")` (a single-component join with a `/` literal — produces mixed separators on Windows) and compares it via `expected_path.display().to_string()`. After production normalises to `/`, the test must also normalise its expected value (e.g., `expected_path.to_string_lossy().replace('\\', "/")`) to compare apples to apples.

**Affected Files:**
- `crates/fdemon-app/src/ide_config/emacs.rs` — production fix in `generate_elisp` (or a new `to_lisp_path` helper); test fix in `test_emacs_merge_produces_absolute_path` to normalise its expected value.

---

### Bug 4: Two VS Code IDE-config tests fail on Windows due to backslash separators in `cwd` field

**Symptom (Windows):** Two tests in `crates/fdemon-app/src/ide_config/vscode.rs` fail with `left: "example\\app3"` vs `right: "example/app3"`:

- `test_compute_cwd_project_is_child_returns_relative_path` (line 305)
- `test_vscode_monorepo_cwd_is_relative_path` (line 408)

**Expected:** The generated `.vscode/launch.json` `cwd` field should use forward slashes, matching VS Code's cross-platform convention for JSON path values.

**Root Cause Analysis:**
1. `compute_cwd(project_root, workspace_root)` returns `rel.to_string_lossy().into_owned()` after `strip_prefix`. On Windows `to_string_lossy()` yields backslashes.
2. VS Code accepts forward slashes in `cwd` on Windows. Forward slashes are also the convention in `.vscode/launch.json` files committed by cross-platform projects.
3. Production code is wrong: it should normalise the relative path to `/` when serialising to JSON.

**Affected Files:**
- `crates/fdemon-app/src/ide_config/vscode.rs` — replace `rel.to_string_lossy().into_owned()` with `.replace('\\', "/")` on the same value (or a more rigorous component-join). Both failing tests are fixed by the single production change.

---

### Bug 5: `state::tests::test_device_cache_does_not_expire` panics on Windows from `Instant - Duration` underflow

**Symptom (Windows):** `overflow when subtracting duration from instant` at `library\std\src\time.rs:445`, originating from `crates/fdemon-app/src/state.rs:1778-1788`.

**Expected:** The test should verify that `get_cached_devices()` continues to return the cached value even after a long simulated interval, on every platform.

**Root Cause Analysis:**
1. The test does `state.devices_last_updated = Some(Instant::now() - Duration::from_secs(60 * 60))`.
2. Windows `Instant` ticks from boot. On a freshly-booted GitHub Actions Windows runner, system uptime is typically minutes, not hours. `Instant::now() - 1h` panics. (macOS `Instant` also ticks from boot, but macOS runners typically have longer uptime by the time this test runs.)
3. Reviewing the production code: `get_cached_devices()` returns `self.device_cache.as_ref()` with **no expiry check**. The cache never expires by design.
4. Therefore the time manipulation in the test is unnecessary — the test could simply call `set_device_cache(...)` followed by `get_cached_devices()` and assert it is `Some(_)`. The simpler test is also a more honest expression of the contract being verified.

**Affected Files:**
- `crates/fdemon-app/src/state.rs:1778-1788` — replace the `Instant::now() - Duration` line with either (a) `Instant::now().checked_sub(Duration::from_secs(3600))` with a sensible fallback if `None`, or (b) drop the time manipulation entirely since `get_cached_devices` has no expiry logic. Recommend (b) — simplest, most truthful.

---

### Bug 6: `actions::ready_check::tests::test_http_check_success` fails on Windows due to single-shot mock server race

**Symptom (Windows):** `assertion failed: result.is_ready()` at `crates/fdemon-app/src/actions/ready_check.rs:398`.

**Expected:** The mock HTTP server should serve every retry attempt that `run_http_check` makes within its 5-second timeout window.

**Root Cause Analysis:**
1. The test spawns a server task that calls `accept()` exactly **once**, writes a 200 response, and exits. `run_http_check` retries every 100ms with a 5s timeout (up to 50 attempts).
2. On Windows, `TcpStream::connect` can return success at the OS level (the SYN is accepted by the TCP stack) before the spawned task has reached `accept()`. The first `try_http_get` then writes the GET request and reads the response — but the read returns empty / `Connection reset` because the server task hasn't yet processed the connection. `try_http_get` returns `Ok(false)` (status-line parse fails).
3. The single-accept server is now consumed. Subsequent retries get `Connection refused`. The check eventually returns `NotReady` and the assertion fails.
4. The same file already has `test_http_check_non_200_retries` (line 412) which uses an `accept()` loop — this is the correct pattern. Test 10 was authored differently.

**Affected Files:**
- `crates/fdemon-app/src/actions/ready_check.rs:376-399` — change the spawned task to accept and respond in a loop (matching the pattern at line 412).

---

### Bug 7: Reviewer comment — inaccurate workflow comment about toolchain pinning

**Symptom:** Copilot reviewer flagged `.github/workflows/ci.yml:33`: a comment near the `dtolnay/rust-toolchain` SHA pin claims pinning "freezes the Rust toolchain version." This is inaccurate — pinning the action SHA only pins the *action code*; the toolchain still resolves `stable` at workflow runtime unless `with: toolchain: <version>` is set or a `rust-toolchain.toml` is checked in.

**Expected:** Either the comment honestly describes the supply-chain benefit (action-code pinning), or the toolchain itself is actually pinned.

**Root Cause Analysis:**
1. The intent of the SHA pin is supply-chain hardening (don't trust mutable tags). It does not pin the rustc version.
2. The MSRV declaration in `Cargo.toml` (`rust-version = 1.77.2`) is currently advisory because CI uses `stable`. Pinning the toolchain to `1.77.2` would be a meaningful policy change but is **out of scope** for this fix — see `workflow/plans/bugs/msrv-is-multiple-of-cleanup/BUG.md` "Further Considerations" for the discussion.
3. Smallest correct fix: revise the comment to accurately describe what the SHA pin does (action-code immutability) without claiming toolchain pinning.

**Affected Files:**
- `.github/workflows/ci.yml:33` (or surrounding lines containing the inaccurate comment).

---

### Bug 8: Reviewer comment — local-verification commands in DEVELOPMENT.md drift from CI

**Symptom:** Copilot reviewer flagged `docs/DEVELOPMENT.md:178`: documented commands omit `--all-targets`, but CI runs `cargo check --workspace --all-targets` and `cargo clippy --workspace --all-targets -- -D warnings`. Contributors running the documented commands locally will still pass tests that fail in CI.

**Expected:** The documented "verification commands" and "quality gate" sections should match the actual CI invocation.

**Root Cause Analysis:**
1. `docs/DEVELOPMENT.md` lines 33-45 ("Verification Commands" + "Full verification") and the per-crate examples list `cargo check --workspace` and `cargo clippy --workspace -- -D warnings`.
2. `.github/workflows/ci.yml` runs the same commands plus `--all-targets` (which extends coverage to integration tests, examples, and benches).
3. This is a docs-only update. The doc_maintainer is the only agent allowed to edit `docs/DEVELOPMENT.md`.

**Affected Files:**
- `docs/DEVELOPMENT.md` — lines 33-45 and any other places that list the workspace-wide commands.

---

### Bug 9: Reviewer comment — Windows `InvalidInput` error message references wrong launch.toml path

**Symptom:** Copilot reviewer flagged `crates/fdemon-daemon/src/process.rs:93`: the Windows-specific error message produced when `Command::spawn` returns `InvalidInput` (post-CVE-2024-24576 escaper rejection) tells the user to "Check launch.toml." The actual path is `.fdemon/launch.toml`.

**Expected:** The user-facing message should reference the correct relative path so the user knows where to look.

**Root Cause Analysis:**
1. `process.rs` was added/edited during PR #38's `windows-flutter-bat-spawn-followup` Wave A to surface a clearer diagnostic when dart-define values contain characters cmd.exe cannot escape.
2. Other UI text in the project (e.g., `docs/DEVELOPMENT.md:280`, `docs/ARCHITECTURE.md` config references) consistently uses the relative path `.fdemon/launch.toml`.
3. Single-line wording fix: replace `Check launch.toml` with `Check .fdemon/launch.toml` (or equivalent — the wording is flexible as long as the path is accurate).

**Affected Files:**
- `crates/fdemon-daemon/src/process.rs:93` (and any sibling line if the message is reused).

---

## Affected Modules

- `crates/fdemon-daemon/src/tool_availability.rs` — Bug 1 (test cfg gate).
- `crates/fdemon-daemon/src/flutter_sdk/locator.rs` — Bug 2 (env-var scrub).
- `crates/fdemon-app/src/ide_config/emacs.rs` — Bug 3 (Lisp-path normalisation + one test fix).
- `crates/fdemon-app/src/ide_config/vscode.rs` — Bug 4 (cwd normalisation).
- `crates/fdemon-app/src/state.rs` — Bug 5 (drop time manipulation).
- `crates/fdemon-app/src/actions/ready_check.rs` — Bug 6 (server accept-loop).
- `.github/workflows/ci.yml` — Bug 7 (comment fix).
- `docs/DEVELOPMENT.md` — Bug 8 (docs-only).
- `crates/fdemon-daemon/src/process.rs` — Bug 9 (error message path fix).

---

## Phases

### Phase 1: Portability + reviewer feedback (single wave) — Should Fix

**Approach:** Eight disjoint-write-file tasks dispatched in parallel under worktree isolation. No task depends on another. The 7 implementor tasks plus 1 doc_maintainer task can all run concurrently.

**Steps (per task — see TASKS.md for the full list):**
1. Apply the targeted fix described in the corresponding Bug section above.
2. Verify per-crate locally:
   - `cargo clippy -p <crate> --all-targets -- -D warnings` exits 0.
   - `cargo test -p <crate>` passes.
   - `cargo fmt --all -- --check` is clean.
3. After all tasks merge, the 3-OS CI matrix (`.github/workflows/ci.yml`) runs and all jobs go green.

**Measurable Outcomes:**
- All 10 failing tests pass on all three OS runners.
- The 3 Copilot review comments are resolved (mark them as resolved on the PR after merge).
- No new test or production regressions introduced.

---

## Edge Cases & Risks

### Path normalisation might affect non-IDE consumers
- **Risk:** Other call sites of `compute_cwd` or `generate_elisp` may rely on the OS-native separator. Forward-slash output could surprise them.
- **Mitigation:** Both functions are called only from IDE config generation paths (`emacs.rs::generate`/`merge_config`, `vscode.rs::generate`). Verify by grepping for the function names before changing them. If any non-IDE caller exists, prefer adding a separate `to_ide_path` helper and only normalise at the IDE-config emission point.

### `#[cfg(target_os = "macos")]` on tests reduces Linux/Windows coverage
- **Risk:** Tests gated to macOS no longer exercise the iOS code path on Linux/Windows.
- **Mitigation:** This is correct — the iOS code path itself is `#[cfg(target_os = "macos")]`-gated. There is nothing to exercise on Linux/Windows. The companion `test_native_logs_available_ios_no_tools` already runs on every platform and verifies the catch-all `false` branch.

### `serial_test::serial` annotation requires the `serial_test` crate
- **Risk:** If `serial_test` is not already a dev-dependency of `fdemon-daemon`, adding `#[serial]` will fail to compile.
- **Mitigation:** Verify the dev-dependency exists. Other tests in `locator.rs` already use `#[serial]` per the codebase researcher's findings, so the dependency is present.

### Windows `Instant` semantics may affect other tests
- **Risk:** Test 9 is the test that triggered the underflow, but other tests may also use `Instant - Duration` and silently break on Windows runners.
- **Mitigation:** A pre-fix grep for `Instant::now()` followed by `-` in test code is part of the implementor's verification checklist. Fix any others encountered with the same pattern.

### HTTP server-loop fix may reveal a real production bug
- **Risk:** If `run_http_check` makes more retries on Windows than on Linux/macOS, the loop fix masks a slower-than-expected client-side retry interval.
- **Mitigation:** The fix addresses the immediate symptom (single-accept server). If the test still flakes after the loop fix, file a follow-up to investigate the connect/read race on Windows TCP behavior.

### MSRV pinning decision deferred
- **Risk:** Bug 7 fixes the comment but does not pin the toolchain. The MSRV remains advisory. A future Rust release could break local builds on the declared MSRV without CI noticing.
- **Mitigation:** Out of scope here. See `workflow/plans/bugs/msrv-is-multiple-of-cleanup/BUG.md` "Further Considerations" for the standalone discussion.

---

## Further Considerations

1. **Should the project add a `rust-toolchain.toml` pinning to 1.77.2?** Discussed in `msrv-is-multiple-of-cleanup`. **Out of scope** for this followup.

2. **Should we add CI assertions that `--all-targets` is used everywhere?** A small `scripts/verify-quality-gate.sh` could parse the workflow YAML and the docs for consistency. **Out of scope** — three lines of docs alignment is the cheapest fix.

3. **Should `compute_cwd` and `to_lisp_path` be moved to a shared `path_utils` module?** The two normalisation sites are simple `.replace('\\', "/")` calls and are unlikely to grow. Keep them inline at each call site — extract only if a third site appears.

---

## Task Dependency Graph

```
Wave 1 (parallel — disjoint write-file sets)
├── 01-fix-ios-tool-availability-tests       (tool_availability.rs)
├── 02-fix-flutter-wrapper-env-leak          (locator.rs)
├── 03-fix-emacs-path-separator              (emacs.rs)
├── 04-fix-vscode-path-separator             (vscode.rs)
├── 05-fix-instant-subtraction               (state.rs)
├── 06-fix-http-mock-server-loop             (ready_check.rs)
├── 07-fix-review-feedback-code              (ci.yml + process.rs)
└── 08-fix-development-doc-commands          (DEVELOPMENT.md — doc_maintainer)
```

No cross-task dependencies — Wave 1 is the only wave.

---

## Success Criteria

### Phase 1 Complete When:

- [ ] CI run on `fix/detect-windows-bat` shows green on all three OS jobs (`ubuntu-latest`, `macos-latest`, `windows-latest`).
- [ ] `cargo test --workspace --all-targets` passes locally on macOS (developer baseline).
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` exits 0.
- [ ] All 10 originally-failing tests pass:
  - [ ] `tool_availability::tests::test_native_logs_available_ios_with_idevicesyslog`
  - [ ] `tool_availability::tests::test_native_logs_available_ios_with_simctl`
  - [ ] `flutter_sdk::locator::tests::test_flutter_wrapper_detection`
  - [ ] `ide_config::emacs::tests::test_emacs_generate_embeds_absolute_path`
  - [ ] `ide_config::emacs::tests::test_emacs_merge_uses_absolute_path`
  - [ ] `ide_config::emacs::tests::test_emacs_merge_produces_absolute_path`
  - [ ] `ide_config::vscode::tests::test_compute_cwd_project_is_child_returns_relative_path`
  - [ ] `ide_config::vscode::tests::test_vscode_monorepo_cwd_is_relative_path`
  - [ ] `state::tests::test_device_cache_does_not_expire`
  - [ ] `actions::ready_check::tests::test_http_check_success`
- [ ] The 3 Copilot review comments on PR #38 are resolved by the corresponding code/doc edits.

---

## Milestone Deliverable

PR #38 turns green on all three CI runners. The 3-OS matrix becomes a useful regression gate going forward (any future Windows path bug, env-var leak, or single-shot server race will be caught by CI before merge). The MSRV pinning question remains open but is documented elsewhere.
