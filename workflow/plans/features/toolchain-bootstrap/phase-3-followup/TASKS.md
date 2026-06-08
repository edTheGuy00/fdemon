# Phase 3 Followup — Review Remediation — Task Index

## Overview

This phase remediates the findings from the Phase 3 code review
(`workflow/reviews/features/toolchain-bootstrap-phase-3/REVIEW.md` +
`ACTION_ITEMS.md`). Every finding was re-verified against the current code by a
research pass before this breakdown; the research corrected three review claims
(noted inline below). The work splits into five focused implementor tasks plus
two small doc tasks.

**Total Tasks:** 7
**Estimated Hours:** 8–13 hours

### Findings → Tasks Map

| Finding | Severity | Task |
|---------|----------|------|
| M2 — duplicated SDK-root resolver (divergence risk) | Major | 01 |
| M1 — `resolve_jdk_home()` unwired | Major | 02 |
| M4 — malformed PathConfig summary string | Major | 02 |
| M3 — rc-file `$`/`"` injection gap + `android_fish_block` | Major | 03 |
| M5 — stale `on_line` doc comment | Major | 03 |
| m1 — license acceptance unverified (`y\n`×20) | Minor | 04 |
| m5 — dead `log_lines` + gratuitous clones | Minor | 04 |
| n1 — temp dir `{pid}` collision | Nitpick | 04 |
| n3 — `jdk_bin` `format!` vs `Path::join` | Nitpick | 04 |
| m2 — gate vs guided-command divergence (no-JDK edge) | Minor | 05 |
| m3 — PathConfig silently omits ANDROID_HOME if run first | Minor | 05 |
| n4 — `state.rs` "task 09" doc ref | Nitpick | 05 |
| n5 — opaque `bottom_area` arithmetic | Nitpick | 05 |
| n6 — re-export gateway for TUI test types | Nitpick | 05 |
| n7 — document `build-tools;<api>.0.0` + build-number maintenance | Nitpick | 06 |
| M2 (doc follow) — ARCHITECTURE.md resolver consolidation | — | 07 |

### Research Corrections (applied to the tasks below)

- **M1** — fix belongs in the **executor** (`actions/mod.rs`, inside `tokio::spawn`),
  **not** the handler: `resolve_jdk_home` does env/filesystem I/O and must not run in
  the pure TEA handler. (Review's "could go in the handler" is wrong.)
- **M2** — platform-default path casing is currently **identical** in both resolvers;
  the fix is **preventive** (single source of truth), not a live-bug fix. The daemon
  `AndroidSdkRoot` newtype does **not** need to be exported — a `PathBuf`-returning
  helper suffices.
- **M3** — the `jdk_dir`→`--jdk-dir=` "injection" is **overstated**: `run_streaming`
  uses exec-style `Command::args` (no shell), so there is no shell-injection vector;
  the `jdk.rs` validation is **defensive only** (newlines/control chars). However the
  research found **`android_fish_block` has the same `"`/`$` exposure** as
  `android_posix_block`, which the review missed — task 03 fixes both.

## Tasks

| # | Task | Status | Depends On | Est. | Agent | Modules |
|---|------|--------|------------|------|-------|---------|
| 01 | [01-consolidate-android-sdk-root-resolver](tasks/01-consolidate-android-sdk-root-resolver.md) | ✅ Done (PASS) | - | 2-3h | implementor | daemon `checks/android.rs`, `checks/mod.rs`, `toolchain/mod.rs`, `lib.rs`; app `actions/mod.rs` |
| 02 | [02-executor-jdk-fallback-and-summary](tasks/02-executor-jdk-fallback-and-summary.md) | ✅ Done (PASS) | 01 | 1-2h | implementor | app `actions/mod.rs` |
| 03 | [03-rcfile-injection-hardening](tasks/03-rcfile-injection-hardening.md) | ✅ Done (PASS) | - | 2-3h | implementor | daemon `path_config.rs`, `jdk.rs` |
| 04 | [04-android-install-license-verify-cleanup](tasks/04-android-install-license-verify-cleanup.md) | ✅ Done (PASS) | - | 2-3h | implementor | daemon `android_install.rs` |
| 05 | [05-wizard-ux-polish](tasks/05-wizard-ux-polish.md) | ✅ Done (PASS) | - | 1-2h | implementor | app `install_wizard/state.rs`, `handler/install_wizard/actions.rs`, `install_wizard/mod.rs`; tui `step_detail.rs`, `widgets/install_wizard/mod.rs` |
| 06 | [06-document-android-config-assumptions](tasks/06-document-android-config-assumptions.md) | ✅ Done (PASS) | - | 0.5h | implementor | `docs/CONFIGURATION.md` |
| 07 | [07-update-architecture-resolver-doc](tasks/07-update-architecture-resolver-doc.md) | ✅ Done (PASS*) | 01 | 0.5h | doc_maintainer | `docs/ARCHITECTURE.md` |

\* Validator returned CONCERN re: an apparent `actions/mod.rs` reversion — confirmed a false
alarm caused by the worktree forking before task 02 landed. The actual squash merge staged
only `docs/ARCHITECTURE.md` + the task file; no code reversion occurred.

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01 | `crates/fdemon-daemon/src/toolchain/checks/android.rs`, `crates/fdemon-daemon/src/toolchain/checks/mod.rs`, `crates/fdemon-daemon/src/toolchain/mod.rs`, `crates/fdemon-daemon/src/lib.rs`, `crates/fdemon-app/src/actions/mod.rs` | — |
| 02 | `crates/fdemon-app/src/actions/mod.rs` | `crates/fdemon-daemon/src/toolchain/jdk.rs`, `crates/fdemon-daemon/src/toolchain/android_install.rs`, `crates/fdemon-app/src/handler/install_wizard/actions.rs` |
| 03 | `crates/fdemon-daemon/src/toolchain/path_config.rs`, `crates/fdemon-daemon/src/toolchain/jdk.rs` | `crates/fdemon-daemon/src/toolchain/process_stream.rs` |
| 04 | `crates/fdemon-daemon/src/toolchain/android_install.rs` | `crates/fdemon-daemon/src/toolchain/process_stream.rs`, `crates/fdemon-daemon/src/toolchain/types.rs` |
| 05 | `crates/fdemon-app/src/install_wizard/state.rs`, `crates/fdemon-app/src/handler/install_wizard/actions.rs`, `crates/fdemon-app/src/install_wizard/mod.rs`, `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs`, `crates/fdemon-tui/src/widgets/install_wizard/mod.rs` | `crates/fdemon-app/src/handler/update.rs` |
| 06 | `docs/CONFIGURATION.md` | `crates/fdemon-daemon/src/toolchain/types.rs` |
| 07 | `docs/ARCHITECTURE.md` | task 01 |

### Overlap Matrix

Wave-peer comparisons (tasks with no dependency edge that may run concurrently):

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|--------------------|--------------------|
| 01 + 03 + 04 + 05 + 06 | None (disjoint across crates/files) | **Parallel (worktree)** |
| 01 + 02 | `crates/fdemon-app/src/actions/mod.rs` | **Sequential (same branch)** — 02 depends on 01 |
| 02 + 07 | None | Parallel (worktree) — both after 01 |
| 03 + 04 | None (`path_config.rs`+`jdk.rs` vs `android_install.rs`) | Parallel (worktree) |

**Key isolation note:** `crates/fdemon-app/src/actions/mod.rs` is the only file
written by more than one task (01 then 02). Task 02 depends on 01, so they run
sequentially on the same branch — no merge conflict. Tasks 03, 04, 05 each own a
disjoint set of files (daemon `path_config`/`jdk` vs daemon `android_install` vs
app/tui `install_wizard`), so they parallelize cleanly with each other and with 01.
Task 03 and 04 both live in `fdemon-daemon/toolchain/` but touch different files,
so they do **not** conflict. Note 01 touches `toolchain/mod.rs` (re-export) and
`lib.rs`; no other task touches those, so the daemon re-export surface has a single
writer.

## Suggested Wave Schedule

- **Wave 1 (parallel worktrees):** 01, 03, 04, 05, 06
- **Wave 2 (parallel, after 01):** 02 (`actions/mod.rs` chain), 07 (doc_maintainer)

## Success Criteria

Phase 3 Followup is complete when:

- [ ] **M2:** A single daemon helper resolves the Android SDK root; the install-time
      path and the check-time `android_sdk_root()` both derive from it; a test asserts
      they agree on identical env/default inputs. The private app-side
      `resolve_android_sdk_root` is deleted.
- [ ] **M1:** When `[toolchain] jdk_path` is unset, the AndroidTools executor falls
      back to `resolve_jdk_home()` so `JAVA_HOME` is exported to `sdkmanager`; a test
      proves the fallback resolves from `JAVA_HOME`.
- [ ] **M4:** The PathConfig completion summary reads cleanly for both Flutter-only
      and Flutter+Android outcomes (no comma-splice, no double spaces); a test asserts
      the combined string.
- [ ] **M3:** rc-file writers no longer emit a path containing `"`/`$` unsafely
      (single-quote escaping or expanded blocklist) for both `android_posix_block`
      **and** `android_fish_block`; the false `posix_export_line` doc comment is
      corrected; `configure_flutter_jdk_dir` defensively validates `jdk_dir`.
- [ ] **M5:** The stale `on_line` doc comment on `configure_flutter_jdk_dir` is fixed.
- [ ] **m1:** `sdkmanager --licenses` output is scanned for a success marker and a
      warning is logged when acceptance can't be confirmed.
- [ ] **m5/n1/n3:** dead `log_lines`/clones removed (or `log_lines` wired into the
      m1 scan), temp dir uses a collision-resistant suffix, `jdk_bin` uses `Path::join`.
- [ ] **m2/m3/n4/n5/n6:** gate and guided-command share one JDK helper; PathConfig
      surfaces an ordering hint when `android_sdk_root` is `None`; doc/comment nits and
      the TUI re-export gateway are addressed.
- [ ] Docs updated (CONFIGURATION.md assumptions; ARCHITECTURE.md resolver note).
- [ ] `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`,
      `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`
      all pass.

## Notes / Backlog (not built this phase)

- **m4 — Windows CI runner:** the highest-blast-radius code (`add_android_env_windows`
  registry writes) is only unit-tested on string constants with no Windows runner, and
  has no rollback on partial failure. Add a Windows CI job for `fdemon-daemon` toolchain
  tests. Tracked, deferred to an infra task.
- **m6 — optional `[toolchain] cmdline_tools_sha256` override:** the cmdline-tools zip
  has no checksum (Google publishes none per build); integrity rests on TLS. A future
  opt-in sha256 override would help enterprise TLS-intercept environments. Tracked,
  deferred to a feature task.
