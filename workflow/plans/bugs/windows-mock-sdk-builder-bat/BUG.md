# Bug: Windows CI portability — final sweep

## Summary

PR #38's `windows-latest` CI has been red for 2 days. Eight separate fix rounds each shipped a small subset of the Windows incompatibilities, exposing the next layer underneath. This plan ends that pattern: it is the result of a single exhaustive audit (covering every previous failed CI run + a forward-looking sweep of test code, production code, and fixture builders) and bundles **all remaining and latent Windows portability issues** into one batch.

After this lands, no further Windows-only fix rounds should be necessary for the surface area covered.

## What's already fixed (verified)

Across previous rounds (commits `54699d7`, `708072a`, `5f235c5`, `da606d3`, `d2dd5c8`, `60024b2`, `5c37220`, `c32d6fd`, `37403aa`, `12f4102`, `5382bbf`, `88e72eb`, `b98dd22`, `ba8aa79`):

| # | Issue | Status |
|---|-------|--------|
| 1 | iOS native-log tests inside non-macOS cfg arms | Fixed (cfg-gated) |
| 2 | `FLUTTER_ROOT` env-var leakage in `test_flutter_wrapper_detection` | Fixed (env scrub + `#[serial]`) |
| 3 | Backslash paths emitted into Emacs/VS Code config | Fixed (slash-normalised) |
| 4 | `Instant::now() - Duration` underflow on fresh-boot Windows | Fixed in 4 sites (`checked_sub`) |
| 5 | TCP accept-once race in `test_http_check_success` | Fixed (loop) |
| 6 | TCP RST-on-close race in `test_http_check_success` | Fixed (drain + shutdown) |
| 7 | `tokio::process::Command` cfg-unguarded imports | Fixed in `emulators.rs`, `process.rs` |
| 8 | macOS clippy 1.95 `manual_checked_ops` lint | Fixed (`checked_div`) |
| 9 | `device_name` parameter only used in macOS-gated block | Fixed (`cfg_attr(allow)`) |
| 10 | `derive_macos_process_name` / `derive_ios_process_name` / `is_ios_simulator` dead-code on non-macOS | Fixed (`cfg_attr(allow(dead_code))`) |
| 11 | `PathPrependGuard` test helper unused on Windows | Fixed (`#[cfg(not(target_os = "windows"))]`) |
| 12 | `url::Url::to_file_path()` rejecting Unix paths on Windows | Fixed (portable `dart_uri_to_path`) |
| 13 | Mock-SDK fixtures inside `crates/fdemon-daemon/src/flutter_sdk/{cache_scanner,locator,types}.rs` missing `flutter.bat` | Fixed |
| 14 | `test_custom_capture_working_dir` using `pwd` + `/tmp` | Fixed (`#[cfg(unix)]`) |
| 15 | 11 PTY E2E tests failing on Windows ConPTY | Fixed (per-test `#[cfg_attr(target_os = "windows", ignore)]`) |

## What's still broken or latent (this plan's scope)

The audit found exactly **5 remaining issues** — three blockers (will fail in the next CI run), two latent (won't fail today but are real bugs that should be batched in to avoid future churn).

### BLOCKER — current red CI

#### B1. `MockSdkBuilder::build()` doesn't write `bin/flutter.bat` on Windows

- **File:** `tests/sdk_detection/fixtures.rs` lines 88–129
- **Impact:** 48 integration tests fail in the current run (`tests/sdk_detection.rs`, `tier1_detection_chain.rs`, `tier1_edge_cases.rs`).
- **Cause:** `MockSdkBuilder::build()` writes `bin/flutter` always but writes `bin/flutter.bat` only when the caller has called `.with_bat_file()`. None of the seven layout helpers (`create_fvm_layout`, `create_asdf_layout`, …) and none of the direct callers do, so on Windows `validate_sdk_path` returns `FlutterNotFound`.
- **Why we missed it:** The earlier round (commit `5382bbf`) only patched the unit-test fixtures inside `crates/fdemon-daemon/src/flutter_sdk/{cache_scanner,locator,types}.rs`. `MockSdkBuilder` lives in `tests/` and was missed.

#### B2. `test_command_check_*` tests use Unix-only shell builtins

- **File:** `crates/fdemon-app/src/actions/ready_check.rs` lines 492–513
- **Impact:** Two tests will fail on Windows when CI reaches them: `test_command_check_succeeds_on_true`, `test_command_check_timeout_on_false`. They invoke `Command::new("true")` / `Command::new("false")` — the POSIX shell builtins, no Windows equivalents.
- **Currently latent only because B1 fails earlier in the test run.** Will become a blocker once B1 is resolved.

#### B3. `native_logs/custom.rs` tests use Unix-only commands

- **File:** `crates/fdemon-daemon/src/native_logs/custom.rs` (10 tests, lines 293–670)
- **Impact:** Ten tests will fail on Windows. They invoke `printf`, `echo`, `yes`, and `printenv` directly via `Command::new`, none of which are native Windows binaries.
- **Tests:** `test_custom_capture_with_echo_command`, `test_custom_capture_process_exit`, `test_custom_capture_shutdown`, `test_custom_capture_with_env`, `test_custom_capture_tag_filtering_exclude`, `test_custom_capture_tag_filtering_include`, `test_stdout_ready_pattern_fires_on_match`, `test_stdout_ready_pattern_no_match_drops_tx`, `test_stdout_ready_pattern_none_no_signal`, `test_create_custom_log_capture_returns_box`.
- **Currently latent only because B1 fails earlier in the test run.** Will become a blocker after B1 + B2 land.

### LATENT — won't break next CI but should ride along

#### L1. `test_http_check_non_200_retries` has the same TCP-RST race we just fixed in `test_http_check_success`

- **File:** `crates/fdemon-app/src/actions/ready_check.rs` lines 409–435
- **Impact:** The mock 503 server uses the old "accept → write → drop socket without reading" pattern. On Windows this RSTs the connection. The test currently passes only because the assertion is `TimedOut`, which is reached either way — but it's now exercising the connection-error code path on Windows, not the 503-retry path it's meant to cover. Mirror the drain + shutdown fix from `test_http_check_success`.

#### L2. PTY tests in `tests/e2e/settings_page.rs` and `debug_settings.rs` lack the Windows `cfg_attr(ignore)` gate

- **Files:** `tests/e2e/settings_page.rs` (all PTY tests), `tests/e2e/debug_settings.rs:11`
- **Impact:** Currently these tests are all marked `#[ignore]` for unrelated reasons, so they don't run on any platform — no current CI failure. But the gating is inconsistent with the 11 sibling PTY tests in `tui_*.rs` that we explicitly Windows-gated. If anyone removes the `#[ignore]` later (e.g. to enable a previously-skipped test), it will fail on Windows immediately. Fix the asymmetry while we're here.

## Out of scope

- **`fs2::FileExt::lock_exclusive()` on Windows config writer** — flagged as LATENT in audit but currently working; not associated with any failing test. Out of scope for this PR.
- **`config/writer.rs::escape_toml_string` emitting backslash entry-point paths** — cross-platform config portability concern (`launch.toml` written on Windows is not byte-identical when round-tripped through macOS). LATENT and not a CI failure. Out of scope; track separately if the user cares about cross-host config sharing.
- **All previously fixed items.** This plan does not re-touch any code listed in "What's already fixed".

## File overlap

| Task | Files Modified |
|------|----------------|
| 01 | `tests/sdk_detection/fixtures.rs` |
| 02 | `crates/fdemon-app/src/actions/ready_check.rs` |
| 03 | `crates/fdemon-daemon/src/native_logs/custom.rs` |
| 04 | `tests/e2e/settings_page.rs`, `tests/e2e/debug_settings.rs` |

Tasks 02 (B2 + L1) are merged into a single task because they both touch `ready_check.rs`. All four tasks write disjoint files — safe to run in parallel under worktree isolation.

## Acceptance

- [ ] `cargo fmt --all -- --check` clean
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean on macOS, Linux, Windows
- [ ] `cargo test --workspace` passes on macOS (1898 + 740 + 372 + 842 + 879 + 80 + … unit + integration tests, 0 failed)
- [ ] CI on `windows-latest` reaches "all 3 OS green" for the SHA after this plan lands
- [ ] No further Windows-only fix rounds required on this branch within the audit's scope
