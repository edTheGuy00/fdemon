# Phase 7 — PR #53 Review Remediation — Task Index

## Overview

These tasks remediate the confirmed findings from a multi-agent adversarial code
review of **PR #53** (`feat/toolchain-bootstrap` → `main`): toolchain bootstrap,
the install wizard, and the Android launch-lifecycle fixes. Eleven specialised
reviewers fanned out across the diff by module cluster (download/extraction,
Flutter install, Android/JDK install, PATH config, prerequisite checks,
toolchain-core + doctor CLI, wizard state machine, wizard handlers/actions,
session lifecycle, SDK locator, TUI widgets); every raw finding was then
independently re-verified by a second agent prompted to **refute** it.

Result: **26 confirmed** findings (2 of 28 raw findings were rejected — the fixed
1.5 GiB disk-budget "defect" is intentional/documented design, and the
"sdkmanager not token-cancellable" claim is refuted because `handle_cancel_step`
already pairs `cancel.cancel()` with `j.abort()` + `kill_on_drop(true)`).
Severity breakdown: **2 HIGH, ~13 MEDIUM, ~11 LOW** (low items are bundled into
their module's task). Review verdict: **request-changes**.

The 26 findings are de-duplicated into **12 tasks** (11 implementation + 1 docs),
ordered HIGH-first.

### Headline items

- **F-PR53-01 (HIGH, concurrency):** a stale **cross-kind** `WizardStepStarted`
  drives the defensive `begin_step` fallback (`handler/install_wizard/actions.rs:363-367`),
  which `take()`s `install_task` without `cancel`/`abort` and bumps `run_seq`. A
  precise Esc+Enter sequence turns the live install into a non-cancellable zombie
  that keeps downloading, holds the RAII install lock, loses its seq-guard backstop,
  and desyncs the UI. The Phase-5 followup fixed the *same-kind* leg; this one was
  missed. `WizardStepStarted` carries no `run_seq`. → Task 01.
- **F-PR53-02 (HIGH, destructive env mutation):** the Windows PATH writer
  round-trips through `[Environment]::Get/SetEnvironmentVariable(...,'User')`, which
  expands `%VAR%` and re-persists as `REG_SZ`, **permanently flattening**
  `REG_EXPAND_SZ` entries (`%USERPROFILE%\bin`, `%JAVA_HOME%\bin`, …) in the user's
  global PATH with no backup (`path_config.rs:606-825`). → Task 02.
- **Recurring MEDIUM theme — download integrity / cross-platform:** Android
  cmdline-tools has no checksum and unconstrained redirects/scheme-downgrade
  (Task 03); cancellation + temp-dir lifecycle gaps in the Flutter install
  (Task 04); POSIX `:` PATH separator on Windows and `/usr` JAVA_HOME (Task 05).

## Finding → Task Map

| Finding | Sev | Area | Task |
|---------|-----|------|------|
| Stale cross-kind `WizardStepStarted` clobbers live cancel token/run_seq | HIGH | concurrency | 01 |
| Windows PATH write flattens `REG_EXPAND_SZ` → `REG_SZ`, destroys `%VAR%` | HIGH | path-config | 02 |
| Android cmdline-tools download has no checksum verification | MEDIUM | download | 03 |
| `download_to_file` allows scheme downgrade + unconstrained redirects | MEDIUM | download | 03 |
| `extract_tar_xz` silently skips traversal/symlink-escape entries | LOW | download | 03 |
| Cancellation ignored during SHA verify + archive extraction | LOW | concurrency | 04 |
| Aborting install races detached extract thread vs `TempDirGuard` removal | MEDIUM | concurrency | 04 |
| Temp-dir guard disarmed before rename → extracted SDK leaked on failure | MEDIUM | correctness | 04 |
| Archive install leaks empty outer temp wrapper dir on success | LOW | correctness | 04 |
| JDK bin prepended to PATH with hardcoded `:` — corrupts Windows PATH | MEDIUM | correctness | 05 |
| `java_home_from_which` returns `/usr` for non-JDK java stub | MEDIUM | correctness | 05 |
| `relocate_cmdline_tools` not atomic — destroys existing latest/ on failure | LOW | error-handling | 05 |
| `apply_report` never resets `execution` → stale Failed/Cancelled view masks re-check | MEDIUM | correctness | 06 |
| rc-file atomic write drops original perms (0600 → 0644) | MEDIUM | security | 07 |
| Fixed temp-file name + unlocked read-modify-write (concurrent clobber) | LOW | concurrency | 07 |
| pkgconf alias path probes `pkg-config` → false GTK/GLU "missing" | MEDIUM | correctness | 08 |
| Rosetta `pgrep oahd` → false "Missing" when daemon idle | MEDIUM | correctness | 08 |
| Dead `xz-utils` alias can never match | LOW | correctness | 08 |
| `fdemon doctor` exit-1 always (Android-not-installed) breaks CI use | MEDIUM | correctness | 09 |
| Top-level run flags before `doctor` silently accepted + ignored | LOW | correctness | 09 |
| `flutter doctor` stderr dropped on substring-`contains` dedup | LOW | correctness | 09 |
| `read_version_file` returns `Ok("")` for blank legacy version file | LOW | correctness | 10 |
| Modern (git-less) SDK reports `channel=None` despite manifest channel | LOW | correctness | 10 |
| `strip_ansi` OSC consumes char after inner ESC without checking `\` | LOW | correctness | 10 |
| VM-service hint cleared by late `app.progress(finished:true)` | LOW | correctness | 11 |
| VM-failure guidance may render above buffered triggering error line | LOW | correctness | 11 |
| (docs) ARCHITECTURE reflects the above behavioural fixes | — | docs | 12 |

## Task Dependency Graph

```
Chain A — app wizard files (handler/install_wizard/actions.rs, install_wizard/state.rs,
          message.rs, actions/mod.rs) — SERIAL
  ┌──────────────────────────────┐   ┌──────────────────────────────┐
  │ 01 WizardStepStarted seq-aware│──▶│ 06 reset execution on        │
  │    (HIGH)                     │   │    apply_report (MEDIUM)     │
  └──────────────────────────────┘   └──────────────────────────────┘

Chain B — daemon download/install files (download.rs, flutter_install.rs, android_install.rs, jdk.rs)
  ┌──────────────────────────────┐   ┌──────────────────────────────┐
  │ 03 download transport +       │──▶│ 04 flutter install cancel +  │
  │    integrity (MED) [download, │   │    temp-dir (MED) [flutter,  │
  │    android_install]           │   │    download]                 │
  └───────────────┬──────────────┘   └──────────────────────────────┘
                  │                   ┌──────────────────────────────┐
                  └──────────────────▶│ 05 android/jdk cross-platform│
                                      │    (MED) [android, jdk]      │
                                      └──────────────────────────────┘
  (04 and 05 are file-disjoint from each other → parallel after 03)

Chain C — path_config.rs — SERIAL
  ┌──────────────────────────────┐   ┌──────────────────────────────┐
  │ 02 Windows PATH REG_EXPAND_SZ │──▶│ 07 rc-file perms + temp name │
  │    (HIGH)                     │   │    (MEDIUM)                  │
  └──────────────────────────────┘   └──────────────────────────────┘

Parallel disjoint (no shared files with chains A/B/C or each other)
  ┌──────────────────────┐ ┌──────────────────────┐ ┌──────────────────────┐ ┌──────────────────────┐
  │ 08 prereq probes (MED)│ │ 09 doctor CLI (MED)  │ │ 10 sdk-locator (LOW) │ │ 11 vm-hint (LOW)     │
  └──────────────────────┘ └──────────────────────┘ └──────────────────────┘ └──────────────────────┘

  ┌──────────────────────────────┐
  │ 12 docs (after 01–11)         │  Agent: doc_maintainer
  └──────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Sev (max) | Est. Hours | Modules |
|---|------|--------|------------|-----------|------------|---------|
| 01 | [01-wizard-step-started-seq-aware](tasks/01-wizard-step-started-seq-aware.md) | ✅ Done | - | HIGH | 4-6h | `handler/install_wizard/actions.rs`, `install_wizard/state.rs`, `message.rs`, `actions/mod.rs` |
| 02 | [02-windows-path-reg-expand-sz](tasks/02-windows-path-reg-expand-sz.md) | ✅ Done | - | HIGH | 4-6h | `toolchain/path_config.rs` |
| 03 | [03-download-transport-integrity](tasks/03-download-transport-integrity.md) | ✅ Done (CONCERN: redirect-bound test gap, non-blocking) | - | MEDIUM | 4-6h | `toolchain/download.rs`, `toolchain/android_install.rs` |
| 04 | [04-flutter-install-cancel-tempdir](tasks/04-flutter-install-cancel-tempdir.md) | ✅ Done (CONCERN: is_multiple_of MSRV note, non-blocking) | 03 | MEDIUM | 4-6h | `toolchain/flutter_install.rs`, `toolchain/download.rs`, `toolchain/android_install.rs` (sig adapt) |
| 05 | [05-android-jdk-cross-platform](tasks/05-android-jdk-cross-platform.md) | ✅ Done (CONCERN: AC3 backup-restore test gap, non-blocking) | 03 | MEDIUM | 3-4h | `toolchain/android_install.rs`, `toolchain/jdk.rs` |
| 06 | [06-reset-execution-on-apply-report](tasks/06-reset-execution-on-apply-report.md) | ✅ Done | 01 | MEDIUM | 2-3h | `install_wizard/state.rs`, `handler/install_wizard/actions.rs` |
| 07 | [07-rc-file-perms-temp-name](tasks/07-rc-file-perms-temp-name.md) | ✅ Done | 02 | MEDIUM | 2-3h | `toolchain/path_config.rs` |
| 08 | [08-prereq-probe-fixes](tasks/08-prereq-probe-fixes.md) | ✅ Done | - | MEDIUM | 2-3h | `toolchain/checks/prerequisites.rs` |
| 09 | [09-doctor-cli-fixes](tasks/09-doctor-cli-fixes.md) | ✅ Done | - | MEDIUM | 3-4h | `src/doctor.rs`, `src/main.rs`, `toolchain/doctor.rs`, `toolchain/mod.rs` |
| 10 | [10-sdk-locator-strip-ansi](tasks/10-sdk-locator-strip-ansi.md) | ✅ Done | - | LOW | 2-3h | `flutter_sdk/types.rs`, `flutter_sdk/locator.rs`, `flutter_sdk/diagnostics.rs` |
| 11 | [11-vm-service-hint-resilience](tasks/11-vm-service-hint-resilience.md) | ✅ Done | - | LOW | 1.5-2h | `session/session.rs`, `handler/session.rs` |
| 12 | [12-update-docs](tasks/12-update-docs.md) | ✅ Done | 01-11 | — | 1-1.5h | `docs/ARCHITECTURE.md` |

**Total Tasks:** 12
**Estimated Hours:** 33–48 hours

## File Overlap Analysis

| Task | Files Modified (Write) |
|------|------------------------|
| 01 | `handler/install_wizard/actions.rs`, `install_wizard/state.rs`, `message.rs`, `actions/mod.rs` |
| 02 | `toolchain/path_config.rs` |
| 03 | `toolchain/download.rs`, `toolchain/android_install.rs` |
| 04 | `toolchain/flutter_install.rs`, `toolchain/download.rs` |
| 05 | `toolchain/android_install.rs`, `toolchain/jdk.rs` |
| 06 | `install_wizard/state.rs`, `handler/install_wizard/actions.rs` |
| 07 | `toolchain/path_config.rs` |
| 08 | `toolchain/checks/prerequisites.rs` |
| 09 | `src/doctor.rs`, `src/main.rs`, `toolchain/doctor.rs`, `toolchain/mod.rs` (exit-aggregation only) |
| 10 | `flutter_sdk/types.rs`, `flutter_sdk/locator.rs`, `flutter_sdk/diagnostics.rs` |
| 11 | `session/session.rs`, `handler/session.rs` |
| 12 | `docs/ARCHITECTURE.md` |

### Overlap Matrix (write-file conflicts)

| Pair | Shared write files | Strategy |
|------|--------------------|----------|
| 01 ↔ 06 | `install_wizard/state.rs`, `handler/install_wizard/actions.rs` | **Sequential (same branch)** — chain A: 01 → 06 |
| 02 ↔ 07 | `toolchain/path_config.rs` | **Sequential (same branch)** — chain C: 02 → 07 |
| 03 ↔ 04 | `toolchain/download.rs` | **Sequential (same branch)** — 04 after 03 |
| 03 ↔ 05 | `toolchain/android_install.rs` | **Sequential (same branch)** — 05 after 03 |
| 04 ↔ 05 | none | **Parallel (worktree)** — after 03 |
| 08 / 09 / 10 / 11 | none (with anything) | **Parallel (worktree)** |
| 12 | none (docs only) | runs last, after 01–11 |

All other task pairs have **no shared write files** → safe to run in parallel
worktrees. Note: `toolchain/mod.rs` is only *read* by other tasks; task 09 should
keep the doctor exit-aggregation change in `src/doctor.rs` and avoid editing the
shared `run_preflight` shape so it stays conflict-free.

## Suggested Wave Schedule

- **Wave 1 (parallel worktrees):** 01 (chain A start), 02 (chain C start),
  03 (chain B start), 08, 09, 10, 11
- **Wave 2:** 06 (after 01), 07 (after 02), 04 (after 03), 05 (after 03) —
  these four are mutually file-disjoint → parallel worktrees
- **Wave 3:** 12 docs (after 01–11) → `doc_maintainer`

**Isolation note:** Chains A (01→06), B (03→{04,05}), and C (02→07) each share
files within the chain — serialise on the same branch, do not run a chain's members
in parallel worktrees. The four disjoint tasks (08–11) and the cross-chain Wave-1
entry points are safe to parallelise.

## Success Criteria

Phase 7 followup is complete when:

- [ ] A stale cross-kind `WizardStepStarted` (mismatched `run_seq`) is a no-op and
      can no longer drop the live install's cancellation token or bump `run_seq`;
      no running install is ever left with `install_task == None` (Task 01).
- [ ] The Windows user PATH retains its `REG_EXPAND_SZ` type and literal `%VAR%`
      tokens after a write; only the new bin dir is appended; injection-safety and
      idempotency are preserved (Task 02).
- [ ] `download_to_file` rejects non-HTTPS URLs and scheme-downgrading/over-long
      redirects; the Android cmdline-tools path verifies a configured checksum
      before extraction; `extract_tar_xz` fails closed on traversal/symlink-escape
      and its doc is corrected (Task 03).
- [ ] A cancel during verify/extract returns promptly without racing cleanup; the
      temp-dir guard is disarmed only after a successful rename (no SDK leak on
      failure) and no empty wrapper dir remains on success (Task 04).
- [ ] The sdkmanager child PATH uses the OS-correct separator; `java_home_from_which`
      rejects the `/usr` stub; `relocate_cmdline_tools` is atomic (backup-restore)
      (Task 05).
- [ ] `apply_report` resets the per-run `execution` so a re-check shows the
      refreshed component list (Task 06).
- [ ] Atomic rc-file writes preserve the original file permissions (0600 stays 0600)
      and use a unique temp file name (Task 07).
- [ ] pkgconf-only Linux installs detect GTK/GLU correctly; installed-but-idle
      Rosetta is not reported Missing; the dead `xz-utils` alias is removed (Task 08).
- [ ] `fdemon doctor` exits 0 for a healthy non-Android Flutter project; doctor-
      incompatible top-level flags are not silently ignored; stderr dedup uses exact
      equality (Task 09).
- [ ] SDK locator falls back to `flutter.version.json` for blank legacy version
      files and reports the manifest channel for git-less installs; `strip_ansi` no
      longer drops a char on malformed OSC (Task 10).
- [ ] The VM-service-unavailable hint survives a late `app.progress(finished:true)`
      (Task 11).
- [ ] `docs/ARCHITECTURE.md` reflects the corrected invariants (Task 12).
- [ ] `cargo fmt --all --check`, `cargo check --workspace --all-targets`,
      `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`
      all pass; no regressions.

## Notes

- **Two HIGH items lead.** 01 and 02 are the block-worthy defects from the
  `request-changes` verdict; schedule them first.
- **Refuted findings (do NOT action):**
  (1) "`download_to_file` budgets a fixed 1.5 GiB regardless of archive size" — this
  is intentional, documented design (the budget covers download + extraction working
  space on the same filesystem); refining to Content-Length would be *less* correct.
  (2) "Cancellation ignored during sdkmanager license/install" — refuted: every
  cancel site pairs `cancel.cancel()` with `j.abort()`, and `run_streaming_with_input`
  sets `kill_on_drop(true)`, so abort drops the future → drops the child → kills
  sdkmanager. Threading the token through those steps is an optional cleanliness
  nit, not a functional defect, and is intentionally out of scope.
- **Low items are bundled by module** (03 carries one low, 04/07/09/10/11 each carry
  lows) to keep one coherent edit surface per file cluster and minimise worktree
  conflicts — not split into separate micro-tasks.
- **Full review artifact:** the verified review output (per-finding evidence,
  verifier reasoning, severities) is the source of record for these tasks.
- **No new keybindings.** `CONFIGURATION.md` is touched only if Task 09 adds a
  `--require-android` flag; `ARCHITECTURE.md` gets the behavioural corrections via
  Task 12 (`doc_maintainer`).
