# Action Items: Toolchain Bootstrap — Phase 3 (Android Tools + JDK)

**Review Date:** 2026-06-04
**Verdict:** ⚠️ NEEDS WORK
**Blocking Issues:** 0 critical, 5 major (M1–M5)

## Critical Issues (Must Fix)
None.

## Major Issues (Should Fix Before Merge)

### M1 — Wire `resolve_jdk_home()` into the AndroidTools dispatch
- **Source:** logic_reasoning_checker (verified)
- **Files:** `crates/fdemon-app/src/handler/install_wizard/actions.rs:140`, `crates/fdemon-daemon/src/toolchain/jdk.rs:30`
- **Problem:** `resolve_jdk_home()` is built/tested but never called; `jdk_path` defaults to `None`, so `sdkmanager` runs with no `JAVA_HOME`. The JDK gate can pass while the install fails to find Java.
- **Required Action:** When `ts.jdk_path` is `None`, fall back to `resolve_jdk_home()` to populate `AndroidStepParams.jdk_path` (in the handler or executor).
- **Acceptance:** With a JDK present only via `JAVA_HOME` (not `[toolchain] jdk_path`), the executor exports `JAVA_HOME`/PATH to `sdkmanager`. Add a test covering the `jdk_path: None` → resolver fallback.

### M2 — Consolidate the two `resolve_android_sdk_root` resolvers
- **Source:** risks_tradeoffs_analyzer (blocking), architecture_enforcer
- **Files:** `crates/fdemon-app/src/actions/mod.rs:1567`, `crates/fdemon-daemon/src/toolchain/checks/android.rs:32`
- **Problem:** Install-time resolver returns a non-existent path (to create it); check-time resolver requires `is_dir()`. Divergence silently breaks "checks flip to Ok after install."
- **Required Action:** Export one daemon helper `resolve_android_sdk_root(Option<&Path>) -> PathBuf` + a thin `exists`-filtering wrapper for checks; consume from both sites.
- **Acceptance:** Both call sites delegate to the shared helper. A test asserts install-time and check-time resolution agree on identical env/default inputs.

### M3 — Close the rc-file path injection gap
- **Source:** security_reviewer (2× MEDIUM), code_quality_inspector (verified)
- **Files:** `crates/fdemon-daemon/src/toolchain/path_config.rs:100,272,324`, `crates/fdemon-daemon/src/toolchain/jdk.rs:67`
- **Problem:** `validate_bin_dir` blocklist omits `$` and `"` (doc comment falsely claims they're blocked); `ANDROID_HOME` is written double-quoted with the raw path. `jdk_dir` is embedded in `--jdk-dir=` with no validation.
- **Required Action:** Add `"` and `$` to the blocklist OR single-quote the `ANDROID_HOME` value via the existing `single_quote_escape`; validate `jdk_dir` (or pass `--jdk-dir` + value as two argv elements); fix the doc comment at `path_config.rs:272`.
- **Acceptance:** A path containing `"` or `$` is rejected (or safely single-quoted). New test covers an injection-bearing SDK root and jdk_dir.

### M4 — Fix the malformed `PathConfig` summary string
- **Source:** code_quality_inspector, logic_reasoning_checker
- **File:** `crates/fdemon-app/src/actions/mod.rs:1093-1101`
- **Problem:** Joining produces run-on, double-spaced English when both Flutter and Android outcomes are present.
- **Required Action:** Collect non-empty clauses into a `Vec<String>`, join with `". "`, append the restart hint; trim trailing spaces from android summary strings.
- **Acceptance:** Summary reads cleanly for (a) Flutter only and (b) Flutter + Android. Add/adjust a test asserting the combined string.

### M5 — Correct the stale `on_line` doc comment
- **Source:** architecture_enforcer, code_quality_inspector
- **File:** `crates/fdemon-daemon/src/toolchain/jdk.rs:53-54`
- **Problem:** Doc describes an `on_line` callback the function does not have.
- **Required Action:** Replace with "Output from `flutter config` is forwarded to the `tracing` debug log."
- **Acceptance:** Doc matches the signature.

## Minor Issues (Track / Fix Soon)

1. **m1** — License acceptance is fixed `y\n`×20 with no acceptance verification; implement the `flutter doctor --android-licenses` fallback and/or scan output for an "accepted" marker. `android_install.rs`.
2. **m2** — Share one helper between the JDK gate (`jdk_status`) and guided-command derivation (`build_steps`) so the no-JDK-entry edge can't promise an unrendered command. `actions.rs` / `state.rs`.
3. **m3** — Surface a "Run Android Tools first" hint when PathConfig runs with `android_sdk_root: None`. `actions.rs:176`.
4. **m4** — Add a Windows CI runner for `fdemon-daemon` toolchain tests; document that `add_android_env_windows` has no rollback on partial failure. `path_config.rs:642`.
5. **m5** — Remove the dead `log_lines` accumulator + `line.clone()`, and the gratuitous `pkg_refs.clone()`. `android_install.rs`.
6. **m6** — Open a tracking issue for an optional `[toolchain] cmdline_tools_sha256` and document the no-checksum tradeoff.

## Nitpicks (Optional)

- Unique temp-dir suffix instead of `{pid}`; `#[serial]` on JDK env tests; `Path::join` for `jdk_bin`; fix "task 09"→"task 07" doc ref; add a derivation comment on `bottom_area` height math; consider extending the `fdemon-app::install_wizard` re-export gateway; document `build-tools;<api>.0.0` + `DEFAULT_CMDLINE_TOOLS_BUILD` maintenance in CONFIGURATION.md.

## Re-review Checklist

After addressing issues, the following must pass:
- [ ] M1–M5 resolved
- [ ] New tests for M1 (jdk fallback), M2 (resolver agreement), M3 (injection rejection), M4 (summary string)
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo check --workspace --all-targets`
- [ ] `cargo test --workspace`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] m1–m6 have tracking issues or are addressed
