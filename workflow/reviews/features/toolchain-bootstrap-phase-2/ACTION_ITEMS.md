# Action Items: Toolchain Bootstrap — Phase 2

**Review Date:** 2026-06-04
**Verdict:** ⚠️ NEEDS WORK
**Blocking Issues:** 2 CRITICAL (must fix before merge to `main`)

## Critical Issues (Must Fix Before Merge)

### C1. Zip-slip / path traversal in `extract_zip`
- **Source:** security_reviewer
- **File:** `crates/fdemon-daemon/src/toolchain/download.rs:158`
- **Problem:** `dest_dir.join(entry.name())` does not validate that the result stays under `dest_dir`. A tampered/malicious archive entry (`../../.bashrc`, absolute path) overwrites arbitrary user files. `zip` 2.x does not sanitize paths.
- **Required Action:** Before writing each entry, reject names containing `..` components or absolute prefixes, OR normalize `out_path` and assert it `starts_with(dest_dir)`. Apply the same guard to directory creation and any symlink entries.
- **Acceptance:** A unit test with a zip containing a `../escape.txt` entry returns `Err` and writes nothing outside `dest_dir`.

### C2. PowerShell code injection in Windows PATH writer
- **Source:** security_reviewer, risks_tradeoffs_analyzer, logic_reasoning_checker
- **File:** `crates/fdemon-daemon/src/toolchain/path_config.rs:332-338`
- **Problem:** The PATH value is interpolated into a `-Command` string with only `'`→`\'` escaping. PowerShell single-quote escaping is `''` (doubling), and backtick / `$(...)` remain live → arbitrary code execution as the user. This path has no runtime test.
- **Required Action:** Pass the value out-of-band: `Command::new("powershell").args([..., "-Command", "[Environment]::SetEnvironmentVariable('PATH', $env:FDEMON_NEW_PATH, 'User')"]).env("FDEMON_NEW_PATH", &new_path)`. Do not interpolate the path into the script string.
- **Acceptance:** A Windows-gated test with a path containing a space and a single quote sets PATH correctly and does not execute injected commands.

## Major Issues (Should Fix)

### M1. Tar traversal / symlink-follow on extract
- **Source:** security_reviewer
- **File:** `crates/fdemon-daemon/src/toolchain/download.rs:227-230`
- **Suggested Action:** Replace `tar::Archive::unpack(dest_dir)` with `unpack_in(dest_dir)`; set `set_preserve_permissions`/`set_unpack_xattrs` deliberately. Add a traversal test fixture.

### M2. Git argument injection via unvalidated `channel`
- **Source:** security_reviewer
- **File:** `crates/fdemon-daemon/src/toolchain/flutter_install.rs:447-457`
- **Suggested Action:** Validate `channel` (`[A-Za-z0-9._-]`, reject leading `-`); add `--` before the branch arg in the `git clone` args.

### M3. Phase label is dead UI (`InstallEvent::Phase` sent as a log line)
- **Source:** code_quality_inspector
- **File:** `crates/fdemon-app/src/actions/mod.rs:901-907`
- **Suggested Action:** Add `Message::WizardStepPhase { kind, label }` + `handle_step_phase` → `set_step_phase`; emit it from the `InstallEvent::Phase` arm. Update ARCHITECTURE.md message inventory.

### M4. `archive_install` ignores configured `channel` (installs stable silently)
- **Source:** code_quality_inspector, logic_reasoning_checker, risks_tradeoffs_analyzer
- **File:** `crates/fdemon-daemon/src/toolchain/flutter_install.rs:386-388,485`
- **Suggested Action:** Thread `target.channel` into the archive path; if only `stable` is resolvable, warn instead of silently downgrading.

### M5. Partial `final_dir` → confusing unretryable rename failure
- **Source:** logic_reasoning_checker (overlaps security TOCTOU)
- **File:** `crates/fdemon-daemon/src/toolchain/flutter_install.rs:311,398`
- **Suggested Action:** Detect a stale/incomplete `final_dir` before rename and either remove it or fail with an actionable message. Correct the "never left partial" docstring.

### M6. No download timeout / retry / resume
- **Source:** risks_tradeoffs_analyzer
- **File:** `crates/fdemon-daemon/src/toolchain/download.rs:47`, `flutter_install.rs:183`
- **Suggested Action:** Add connect/idle timeouts (mirror `version_check.rs`), a bounded retry with backoff, and download to a `.part` file renamed on success.

### M7. `extract_tar_xz` buffers full archive in RAM (OOM risk)
- **Source:** code_quality_inspector, risks_tradeoffs_analyzer, logic_reasoning_checker
- **File:** `crates/fdemon-daemon/src/toolchain/download.rs:222-233`
- **Suggested Action:** Stream via `lzma-rs` `XzDecoder` into `tar::Archive` (the `stream` feature is already enabled). If deferred, document a minimum-RAM requirement and prefer git clone on Linux.

### M8. O(n) log-tail eviction (`Vec::remove(0)`)
- **Source:** code_quality_inspector
- **File:** `crates/fdemon-app/src/install_wizard/state.rs:130-133` (field in `install_wizard/types.rs`)
- **Suggested Action:** Change `log_tail` to `VecDeque<String>`; use `pop_front`/`push_back`. Renderer already iterates by reference.

### M9. No concurrent-install lock on shared `final_dir`
- **Source:** risks_tradeoffs_analyzer
- **File:** `crates/fdemon-daemon/src/toolchain/flutter_install.rs:398`
- **Suggested Action:** Advisory lockfile under `install_root` (also guards against racing `fvm`), or fail fast if `final_dir` appears mid-install. At minimum document the assumption.

### M10. POSIX/fish rc-file shell injection via crafted path
- **Source:** security_reviewer
- **File:** `crates/fdemon-daemon/src/toolchain/path_config.rs:123-130`
- **Suggested Action:** Reject `bin_dir` containing newlines / shell metacharacters before writing; single-quote the `fish_add_path` argument.

## Minor Issues (Consider Fixing)

- **m1.** Clear `installed_sdk_path` after a successful PathConfig run, or document session-precedence — `handler/install_wizard/actions.rs` [logic_reasoning_checker M2].
- **m2.** Use `#[cfg(windows)]`/`#[cfg(not(windows))]` in `home_dir()` — `path_config.rs:362` [code_quality_inspector].
- **m3.** Guard `FVM_CACHE_PATH` with `is_absolute()` — `flutter_install.rs:92-100` [security_reviewer].
- **m4.** Capture `HostArch::detect()` once — `flutter_install.rs:484-490` [code_quality_inspector, logic_reasoning_checker].
- **m5.** Handle macOS bash `.bash_profile`/`.profile`; surface which rc file was written — `path_config.rs:70` [risks_tradeoffs_analyzer].
- **m6.** Distinguish "Installed (precache incomplete)" status / keep warning sticky — [risks_tradeoffs_analyzer].
- **m7.** Unify or remove `#[cfg(test)]` `fence_already_has_dir` — `path_config.rs:176` [code_quality_inspector].
- **m8.** `debug!`-log the best-effort `remove_file` cleanup — `path_config.rs:257` [code_quality_inspector].
- **m9.** Document SHA-256 = corruption guard, not CDN-MITM defense — `flutter_install.rs:178` [security_reviewer].
- Nitpicks: `RESULT_SUMMARY_HEIGHT` const (`progress.rs`); `// EXCEPTION:` annotation on the test Cell write (`state.rs:501`); derive `Copy` on `HostPlatform`; prefer `fdemon_app::install_wizard` re-export in TUI tests; confirm ANSI sanitization of streamed logs.

## Re-review Checklist

After addressing issues, the following must pass:
- [ ] C1 fixed: zip-slip traversal test passes; no writes escape `dest_dir`.
- [ ] C2 fixed: Windows PATH value passed via env, not interpolated; quote/space test passes.
- [ ] M1, M2 fixed: tar `unpack_in` + channel validation with `--` terminator.
- [ ] M3–M5 fixed (user-visible correctness): phase label live, channel honored on archive path, partial-install handled.
- [ ] M6–M10 fixed or filed as a tracked follow-up task with rationale.
- [ ] Full quality gate passes: `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`.
- [ ] New tests added for each CRITICAL/MAJOR fix (traversal fixtures, Windows escaping, channel validation, partial-dir, idempotent retry).
