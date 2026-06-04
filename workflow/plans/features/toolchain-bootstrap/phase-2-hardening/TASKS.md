# Phase 2 Hardening — Security & Robustness Remediation — Task Index

## Overview

This phase remediates the findings from the Phase 2 code review
(`workflow/reviews/features/toolchain-bootstrap-phase-2/REVIEW.md` +
`ACTION_ITEMS.md`). Phase 2 shipped a managed Flutter SDK installer that downloads
and extracts external archives, runs `git`/`flutter` subprocesses, and writes to the
user's shell config — so the review's two **CRITICAL** findings (zip-slip arbitrary
file overwrite, PowerShell code injection) are merge-blocking, and several **MAJOR**
findings harden the same external-input threat surface or fix user-visible correctness.

No new runtime dependencies are introduced: timeouts/retries are hand-rolled on the
existing `reqwest` clients, and the concurrent-install guard uses an atomic
`create_new` lockfile.

**Total Tasks:** 6 (5 implementation + 1 doc)
**Estimated Hours:** 16–22 hours

## Severity → Task Map

| Finding | Severity | Task |
|---------|----------|------|
| C1 zip-slip path traversal (`download.rs`) | 🔴 CRITICAL | 01 |
| M1 tar traversal/symlink follow (`download.rs`) | 🟠 MAJOR (security) | 01 |
| M7 full-archive-in-RAM xz decode (`download.rs`) | 🟠 MAJOR | 01 |
| M6a download timeout/retry/`.part` (`download.rs`) | 🟠 MAJOR | 01 |
| C2 PowerShell injection (`path_config.rs`) | 🔴 CRITICAL | 03 |
| M10 POSIX/fish rc shell injection (`path_config.rs`) | 🟠 MAJOR (security) | 03 |
| M2 git arg injection via `channel` (`flutter_install.rs`) | 🟠 MAJOR (security) | 02 |
| M4 archive path ignores `channel` (`flutter_install.rs`) | 🟠 MAJOR | 02 |
| M5 partial `final_dir` rename failure (`flutter_install.rs`) | 🟠 MAJOR | 02 |
| M9 no concurrent-install lock (`flutter_install.rs`) | 🟠 MAJOR | 02 |
| M6b manifest fetch timeout (`flutter_install.rs`) | 🟠 MAJOR | 02 |
| M3 phase label is dead UI (`actions/mod.rs` + msg) | 🟠 MAJOR | 04 |
| M8 O(n) `Vec::remove(0)` log tail (`install_wizard`) | 🟠 MAJOR | 05 |
| m1 stale `installed_sdk_path` not cleared | 🟡 MINOR | 04 |
| m2 `home_dir()` cfg fragility | 🟡 MINOR | 03 |
| m3 `FVM_CACHE_PATH` not absolute-checked | 🟡 MINOR | 02 |
| m4 `HostArch::detect()` called twice | 🟡 MINOR | 02 |
| m5 macOS bash `.bash_profile`/`.profile` gap | 🟡 MINOR | 03 |
| m8 swallowed `remove_file` error (no debug log) | 🟡 MINOR | 03 |
| LOW-1 ANSI escapes in streamed log lines | 🔵 NITPICK | 05 |
| nit: `RESULT_SUMMARY_HEIGHT` const | 🔵 NITPICK | 05 |
| nit: `Copy` on `HostPlatform`; SHA-from-same-server doc note | 🔵 NITPICK | 02 |

**Deferred (not in scope — tracked as future enhancements):** m6 "Installed (precache
incomplete)" status badge; `EXCEPTION:` annotation on the test Cell write; preferring
the `fdemon_app::install_wizard` re-export in TUI widget tests. See REVIEW.md.

## Task Dependency Graph

```
Wave 1 (parallel — disjoint files, all worktree-isolated)
┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐
│ 01 download.rs   │ │ 02 flutter_      │ │ 03 path_config.rs│ │ 04 wizard phase  │ │ 05 log-tail +    │
│   extraction +   │ │    install.rs    │ │   PowerShell +   │ │   label + stash  │ │   ANSI-safe      │
│   net robustness │ │   security/corr. │ │   shell inject   │ │   clear          │ │   render         │
└────────┬─────────┘ └────────┬─────────┘ └────────┬─────────┘ └────────┬─────────┘ └────────┬─────────┘
         └────────────────────┴───────────┬────────┴────────────────────┴────────────────────┘
                                           ▼
                              ┌──────────────────────────┐
                              │ 06 ARCHITECTURE.md        │ (doc_maintainer; after 01–05)
                              └──────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 01 | [01-harden-archive-extraction](tasks/01-harden-archive-extraction.md) | ✅ Done (CONCERN logged) | - | 4-5h | `toolchain/download.rs` |
| 02 | [02-harden-install-flow](tasks/02-harden-install-flow.md) | ✅ Done | - | 4-6h | `toolchain/flutter_install.rs` |
| 03 | [03-harden-path-config](tasks/03-harden-path-config.md) | ✅ Done | - | 3-4h | `toolchain/path_config.rs` |
| 04 | [04-wizard-phase-label-and-stash](tasks/04-wizard-phase-label-and-stash.md) | ✅ Done | - | 2-3h | `message.rs`, `actions/mod.rs`, `handler/install_wizard/actions.rs`, `handler/update.rs` |
| 05 | [05-bounded-log-tail-and-ansi](tasks/05-bounded-log-tail-and-ansi.md) | ✅ Done | - | 2-3h | `install_wizard/types.rs`, `install_wizard/state.rs`, `widgets/install_wizard/progress.rs` |
| 06 | [06-update-architecture-doc](tasks/06-update-architecture-doc.md) | ✅ Done (fixed after FAIL) | 01,02,03,04,05 | 1h | `docs/ARCHITECTURE.md` |

> **Wave 1 validation note (Task 01 — CONCERN):** Two acceptance items landed as documented PARTIAL, security outcome correct, non-blocking:
> - **M1 (tar traversal):** `tar` 0.4 has no `Archive::unpack_in`; implementor used `Archive::unpack` which delegates to `Entry::unpack_in` and *skips* (not errors on) traversal entries. No file escapes `dest_dir`; fixture test passes.
> - **M6a (download timeout):** `reqwest::timeout()` is a total-request cap, not a per-chunk idle/stall timer. A future hardening pass could add per-chunk idle detection (needs tower middleware).

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01 | `crates/fdemon-daemon/src/toolchain/download.rs` | `toolchain/types.rs` (`DownloadProgress`) |
| 02 | `crates/fdemon-daemon/src/toolchain/flutter_install.rs` | `toolchain/download.rs`, `toolchain/process_stream.rs`, `toolchain/types.rs` (read-only, existing APIs) |
| 03 | `crates/fdemon-daemon/src/toolchain/path_config.rs` | `toolchain/types.rs` (`HostShell`, `HostPlatform`) |
| 04 | `crates/fdemon-app/src/message.rs`, `crates/fdemon-app/src/actions/mod.rs`, `crates/fdemon-app/src/handler/install_wizard/actions.rs`, `crates/fdemon-app/src/handler/update.rs` | `install_wizard/state.rs` (existing `set_step_phase`), `install_wizard/types.rs` (`WizardStepKind`) — read-only |
| 05 | `crates/fdemon-app/src/install_wizard/types.rs`, `crates/fdemon-app/src/install_wizard/state.rs`, `crates/fdemon-tui/src/widgets/install_wizard/progress.rs` | `fdemon-app::install_wizard` re-exports |
| 06 | `docs/ARCHITECTURE.md` | task files 01–05 |

### Overlap Matrix

Wave-peer comparisons (Wave 1 tasks have no dependency edges between them):

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|--------------------|
| 01 + 02 | None (02 reads `download.rs` only — existing API, no new symbol needed) | Parallel (worktree) |
| 01 + 03 | None | Parallel (worktree) |
| 02 + 03 | None | Parallel (worktree) |
| 04 + 05 | None — 04 touches `message.rs`/`actions/mod.rs`/`handler/*`; 05 touches `install_wizard/{types,state}.rs` + `progress.rs`. `set_step_phase` already exists (Phase 2 task 07), so 04 does NOT modify `state.rs`. | Parallel (worktree) |
| 01/02/03 + 04/05 | None (daemon crate vs app/tui crates) | Parallel (worktree) |

**Key isolation note:** All five implementation tasks write **disjoint file sets** and
can run concurrently in worktrees. Task 02 reads `download.rs` but only uses its
**existing** public API (`download_to_file`, `extract_archive`, `verify_sha256`) whose
signatures are unchanged by Task 01 — so no dependency edge is needed. Each of 01/02
applies its own `reqwest` timeout/retry to its own client (small, intentional
duplication) to keep the tasks independent. Task 06 (doc) is the only sequential step.

## Success Criteria

Phase 2 Hardening is complete when:

- [ ] **C1:** A crafted zip with a `../escape` or absolute-path entry is rejected; no
      file is written outside `dest_dir`. Unit test with a malicious fixture passes.
- [ ] **C2:** The Windows PATH value is passed to PowerShell out-of-band (env var), not
      interpolated into the `-Command` string; a path containing a space and a single
      quote round-trips correctly (Windows-gated test).
- [ ] **M1:** Tar extraction uses traversal-safe unpacking (`unpack_in`) and does not
      follow symlinks out of `dest_dir`; traversal fixture test passes.
- [ ] **M2:** `channel` is validated (`[A-Za-z0-9._-]`, no leading `-`) and `git clone`
      uses a `--` option terminator; an injection-shaped channel is rejected.
- [ ] **M3:** During a real install the wizard's phase row shows the live phase
      ("Downloading", "Verifying", "Extracting", "Cloning") via a `WizardStepPhase`
      message routed to `set_step_phase` — not a `[label]` log line.
- [ ] **M4:** The archive install path honors the configured `channel` (or warns when it
      can only resolve stable) instead of silently installing stable.
- [ ] **M5:** A pre-existing incomplete `final_dir` is handled (removed before rename or
      surfaced as an actionable, retryable error) — no opaque `ENOTEMPTY`.
- [ ] **M6:** Download and manifest-fetch clients have connect/idle timeouts and a
      bounded retry; partial downloads use a `.part` file renamed on success.
- [ ] **M7:** `.tar.xz` extraction streams (no full-archive `Vec<u8>` buffer).
- [ ] **M8:** `log_tail` uses `VecDeque` with O(1) eviction.
- [ ] **M9:** Concurrent installs into the same `final_dir` are guarded by an advisory
      lockfile (or fail fast with a clear message).
- [ ] **M10:** rc-file writes reject paths with newlines/shell metacharacters and the
      `fish_add_path` argument is single-quoted.
- [ ] All new code is unit-tested (malicious-archive fixtures, channel validation,
      Windows escaping, partial-dir, idempotent retry, VecDeque bound). Existing tests
      pass; no regressions.
- [ ] `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`,
      `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` all pass.

## Notes

- **Scope discipline:** This phase fixes review findings only; it does not add new
  product features. The deferred items (m6 precache-status badge, etc.) are explicitly
  out of scope and remain in REVIEW.md for a future pass.
- **No new dependencies:** timeouts/retry hand-rolled on `reqwest`; concurrent-install
  lock via `OpenOptions::new().create_new(true)` lockfile (no `fs2`/`fd-lock` dep).
- **TEA purity preserved:** all I/O stays in `fdemon-daemon::toolchain` and
  `actions/mod.rs` spawned tasks; handlers stay pure (mirrors Phase 2).
- **Suggested wave schedule:** Wave 1 (parallel worktrees): 01, 02, 03, 04, 05.
  Wave 2: 06 (doc).
