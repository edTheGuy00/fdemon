# Code Review: Toolchain Bootstrap — Phase 2 (Managed Flutter SDK Install + PATH Config)

**Review Date:** 2026-06-04
**Branch:** `feat/toolchain-bootstrap`
**Diff Base:** `c94660a..HEAD` (12 tasks, ~5,700 insertions across 4 crates)
**Change Type:** Feature implementation
**Task Index:** `workflow/plans/features/toolchain-bootstrap/phase-2/TASKS.md`

## Overall Verdict: ⚠️ NEEDS WORK — 2 merge-blocking CRITICAL security findings

The feature is architecturally clean, idiomatic, and exceptionally well unit-tested. Layer
boundaries and TEA purity are fully respected (all network/FS/process I/O is isolated in
`fdemon-daemon::toolchain` and dispatched from `actions/mod.rs` spawned tasks; handlers are
pure). **However, the security review found two CRITICAL vulnerabilities that must be fixed
before merge** — a zip-slip path traversal in archive extraction and a PowerShell code-injection
in the Windows PATH writer — plus argument-injection and tar-traversal hardening gaps. The logic,
quality, and risk reviews independently surfaced several MAJOR robustness and correctness issues.

| Agent | Verdict |
|-------|---------|
| architecture_enforcer | ✅ APPROVED (1 nitpick) |
| code_quality_inspector | ⚠️ CONCERNS (3 major) |
| logic_reasoning_checker | ⚠️ CONCERNS (2 major) |
| risks_tradeoffs_analyzer | ⚠️ CONCERNS (1 critical) |
| security_reviewer | ⚠️ CONCERNS — **2 CRITICAL must-fix-before-merge** |

Because the security review identifies confirmed CRITICAL vulnerabilities in code that downloads
and extracts external archives and writes to user shell config, those items are **blocking**.
Everything else is non-blocking but should be tracked. See `ACTION_ITEMS.md` for the prioritized fix list.

---

## What's Strong

- **Layering & TEA purity (architecture_enforcer):** Every I/O call (`reqwest` download, `git clone`,
  archive extract, rc-file write, `flutter precache`) lives in `fdemon-daemon::toolchain` and is invoked
  only inside `tokio::spawn`/`spawn_blocking` from `actions/mod.rs`. Handlers in
  `handler/install_wizard/actions.rs` are pure. TUI consumes daemon display types via the approved
  `fdemon-app::install_wizard` re-export. No new network I/O leaked into `fdemon-app`.
- **Testing (code_quality_inspector: 5/5):** wiremock-backed download tests, golden-file idempotency
  tests for rc writes, SHA-verify/extract round-trips, exhaustive executor dispatch tests.
- **Documentation:** module `//!` headers, `///` on public items, named layout constants with derivation
  comments. ARCHITECTURE/CONFIGURATION/KEYBINDINGS docs were updated in-phase (tasks 11/12).
- **Supply-chain hygiene:** minimized features (`zip` deflate-only, `tar` no-xattr, pure-Rust `lzma-rs`),
  MSRV-aware pins, `kill_on_drop(true)` on streamed children, merged stdout/stderr to avoid pipe deadlock.
- **Correct security primitives where present:** HTTPS via rustls; SHA-256 verified *before* extraction;
  subprocesses invoked with discrete args (no shell string).

---

## Consolidated Findings (deduplicated across agents)

### 🔴 CRITICAL — Blocking merge

**C1. Zip-slip / path traversal in `extract_zip`**
`crates/fdemon-daemon/src/toolchain/download.rs:158`
[Source: security_reviewer]
`dest_dir.join(entry.name())` is written without verifying the result stays under `dest_dir`. A
malicious/tampered archive entry (`../../.bashrc`, absolute path) can overwrite arbitrary user files.
The `zip` 2.x crate does not sanitize paths — the caller must. Reject entries containing `..` or
absolute prefixes, or assert `out_path` starts with a normalized `dest_dir`.

**C2. PowerShell code injection in Windows PATH writer**
`crates/fdemon-daemon/src/toolchain/path_config.rs:332-338`
[Source: security_reviewer (CRITICAL-2), risks_tradeoffs_analyzer (CRITICAL-3), logic_reasoning_checker (m4)]
The PATH update interpolates the install path into a `-Command` string, escaping only `'` as `\'`.
PowerShell single-quote escaping is doubling (`''`), not backslash — and backtick/`$(...)` remain live.
A path with PS metacharacters executes arbitrary code as the user. **Fix:** pass the value out-of-band
via the environment (`.env("FDEMON_NEW_PATH", &new_path)` + reference `$env:FDEMON_NEW_PATH`), never inline.
This is also the only Windows path with no runtime test.

### 🟠 MAJOR — Should fix before merge (security hardening + correctness)

**M1. Tar path-traversal / symlink-following on extract**
`crates/fdemon-daemon/src/toolchain/download.rs:227-230` [Source: security_reviewer HIGH-3]
Uses `tar::Archive::unpack(dest_dir)`. Switch to `unpack_in(dest_dir)` (explicit traversal protection)
and disable xattr/permission preservation unless required. Symlink entries can otherwise escape `dest_dir`.

**M2. Git argument injection via unvalidated `channel`**
`crates/fdemon-daemon/src/toolchain/flutter_install.rs:447-457` [Source: security_reviewer HIGH-2]
`channel` (free-form TOML) is passed to `git clone -b <channel>`. A value like `--upload-pack=…` or
`--config core.askpass=…` is interpreted as a git option (known RCE vectors). Validate the charset
(`[A-Za-z0-9._-]`, reject leading `-`) and add `--` before the branch arg.

**M3. `InstallEvent::Phase` forwarded as a log line — phase label is dead UI**
`crates/fdemon-app/src/actions/mod.rs:901-907` [Source: code_quality_inspector #3]
The executor maps `Phase(label)` to `WizardStepLog`, so `StepExecution::phase_label` is never set and
the `StepProgress` phase row always shows the default "Running…". `set_step_phase` exists and is tested
but unreachable. Add a `WizardStepPhase { kind, label }` message + `handle_step_phase` and emit it.

**M4. `archive_install` ignores `target.channel` (silently installs stable)**
`crates/fdemon-daemon/src/toolchain/flutter_install.rs:386-388,485`
[Source: code_quality_inspector #10, logic_reasoning_checker, risks_tradeoffs_analyzer #6]
The git path honors `channel`; the archive fallback always calls `resolve_stable(...)`. A user with
`channel = "beta"` and no `git` silently gets stable. Thread `target.channel` into the archive path or warn.

**M5. Partial/orphaned `final_dir` produces a confusing, unretryable rename failure**
`crates/fdemon-daemon/src/toolchain/flutter_install.rs:311,398`
[Source: logic_reasoning_checker M1; overlaps security MEDIUM-3 TOCTOU]
The already-installed short-circuit needs both `final_dir` and `bin/flutter`. If a prior install left an
incomplete `final_dir`, the final `rename` fails with `ENOTEMPTY` ("Directory not empty") and the SDK
(fetched into temp) is deleted by cleanup. Docstring claims `final_dir` is never partial. Remove a stale
`final_dir` before rename, or detect and surface an actionable "remove incomplete install at <path>" message.

**M6. No download timeout / retry / resume**
`crates/fdemon-daemon/src/toolchain/download.rs:47`, `flutter_install.rs:183` [Source: risks_tradeoffs_analyzer #1]
The download and manifest clients set no timeout — a stalled socket hangs the wizard indefinitely; a single
dropped stream aborts the whole install from byte 0. `version_check.rs` already establishes a 3s-timeout
discipline that wasn't carried over. Add connect/idle timeouts and a bounded retry; download to `.part`.

**M7. `extract_tar_xz` buffers the entire decompressed archive in RAM**
`crates/fdemon-daemon/src/toolchain/download.rs:222-233`
[Source: code_quality_inspector #5, risks_tradeoffs_analyzer #4, logic_reasoning_checker m2]
`lzma_rs::xz_decompress` decodes the full ~1GB+ tar into a `Vec<u8>` before unpacking → OOM risk on the
RAM-constrained hosts most likely to hit the archive path (containers/CI on Linux). `spawn_blocking` does
not mitigate this. Stream via `lzma-rs`'s `XzDecoder`/`XzStreamDecoder` (the `stream` feature is already enabled).

**M8. `Vec::remove(0)` for log-tail eviction (O(n))**
`crates/fdemon-app/src/install_wizard/state.rs:130-133` [Source: code_quality_inspector #1]
`push_step_log` front-removes on a `Vec`, shifting up to 199 elements per output line. Use `VecDeque<String>`
(O(1) `pop_front`/`push_back`). Bounded today, but the wrong structure and a latent issue if `MAX_LOG_TAIL` grows.

**M9. No concurrent-install / cross-process lock on `final_dir`**
`crates/fdemon-daemon/src/toolchain/flutter_install.rs:398` [Source: risks_tradeoffs_analyzer #2]
PID-suffixed temp dirs disambiguate the temp area but the rename target (`~/fvm/versions/stable`, shared with
`fvm`) is unguarded. Two fdemon instances (or a racing `fvm`) can collide. Add an advisory lockfile under
`install_root` or fail fast; at minimum document the unsupported-concurrency assumption.

**M10. POSIX/fish rc-file shell injection via crafted path**
`crates/fdemon-daemon/src/toolchain/path_config.rs:123-130` [Source: security_reviewer MEDIUM-1]
The export line is written verbatim; a path containing a newline + command poisons `.bashrc`/`.zshenv` and
executes on next shell start (confused-deputy via a repo-checked-in `.fdemon/config.toml`). `fish_add_path`
is unquoted. Reject newlines/metacharacters in `bin_dir`; single-quote the fish argument.

### 🟡 MINOR — Fix soon

- **m1. `installed_sdk_path` stale-stash hazard** — `handler/install_wizard/actions.rs` [logic M2]: never cleared after a PathConfig run; a later PathConfig can re-add a stale dir over the authoritative `settings.flutter.sdk_path`. Clear on consume or document the session-precedence intent.
- **m2. `home_dir()` cfg fragility** — `path_config.rs:362` [code_quality #2]: use idiomatic `#[cfg(windows)]`/`#[cfg(not(windows))]` instead of `target_os` string comparison.
- **m3. `FVM_CACHE_PATH` not checked for absolute path** — `flutter_install.rs:92-100` [security MEDIUM-2]: a relative value resolves against CWD; guard with `is_absolute()`.
- **m4. `HostArch::detect()` called twice** — `flutter_install.rs:484-490` [code_quality #4, logic note]: capture once (`let arch = …`).
- **m5. macOS bash login-shell gap** — `path_config.rs:70` [risks #7]: bash on macOS sources `.bash_profile`/`.profile`, not `.bashrc`; PATH write may silently not take effect. Unhandled shells (nushell/elvish) hard-error.
- **m6. Precache failure reported as "Completed"** — [risks questionable decision]: green checkmark can hide a half-provisioned SDK; surface "Installed (precache incomplete)" or keep the warning sticky in the detail pane.
- **m7. `fence_already_has_dir` is `#[cfg(test)]`-only** — `path_config.rs:176` [code_quality #6]: prod re-implements the predicate inline; unify or remove.
- **m8. Swallowed `remove_file` error without trace** — `path_config.rs:257` [code_quality #7]: add a `debug!` for the best-effort cleanup (consistent with `flutter_install.rs`).
- **m9. SHA-256 hash and payload from same server** — `flutter_install.rs:178-225` [security HIGH-1]: inherent to Flutter's release infra; document that the hash guards corruption, not a CDN-level MITM.

### 🔵 NITPICK

- Magic `1` for result-summary row height in `progress.rs:232-276` — add `RESULT_SUMMARY_HEIGHT` const [code_quality #13].
- Test write to `last_known_visible_height` Cell without `// EXCEPTION:` annotation `state.rs:501` [code_quality #14].
- `HostPlatform` could derive `Copy` (no heap data) — drops a `.clone()` [code_quality #12].
- TUI widget tests import `fdemon_daemon::toolchain` directly; the 4 re-exported types could come via `fdemon_app::install_wizard` for consistency (test-only; dev-dep is correct) [architecture nitpick].
- Streamed git/flutter output may carry ANSI escapes into `WizardStepLog`; confirm the progress widget sanitizes before rendering [security LOW-1].

---

## Documentation Freshness

✅ Handled in-phase: `ARCHITECTURE.md` (task 11), `CONFIGURATION.md` + `KEYBINDINGS.md` (task 12) were all
updated and validated. New deps in `Cargo.toml` are reflected. One **pre-existing** carryover (not a Phase 2
regression): `ARCHITECTURE.md` labels `handler/install_wizard/mod.rs` as "Navigation…" when it's a re-export
shim and omits `navigation.rs` — already logged as a follow-up in TASKS.md.

If the new `WizardStepPhase` message (M3) is added, update the Message inventory in `ARCHITECTURE.md`.

---

## Recommendation

Address **C1 and C2 before merging to `main`** (confirmed CRITICAL security vulnerabilities in
external-input handling). Strongly recommend also fixing **M1, M2** (remaining injection/traversal
hardening) in the same pass since they share the same threat surface and are low-effort. The remaining
MAJOR items (M3–M10) are robustness/correctness and can be a fast follow if a tracked task is filed,
but M3/M4/M5 are user-visible correctness bugs worth fixing now. See `ACTION_ITEMS.md`.
