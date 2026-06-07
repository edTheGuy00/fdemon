# Flutter PATH Not Configured — Task Index

## Overview

Fix the confirmed bug that a managed Flutter (and Android) install via the wizard
**never writes the SDK `bin` dir to the shell rc file** — PathConfig is only a
manual step, so `flutter` ends up off the PATH for new shells. See
[`BUG.md`](BUG.md) for the full root-cause analysis and evidence map.

Approved scope:

- **Auto-config after both FlutterSdk and AndroidTools** installs (Task 01).
- **Include test-isolation hardening** so the suite can never write to a real
  `~/.zshenv`, plus Android temp-dir hygiene (Task 02).
- **Defer** the optional "self-heal stale fence block whose target is missing"
  (Fix 2 in BUG.md) — Task 01's idempotent overwrite covers the common case.

## Finding → Task Map

| Finding | Sev | Area | Task |
|---|---|---|---|
| FlutterSdk/AndroidTools completion never triggers the PATH write (PathConfig manual-only) | MAJOR | wizard chain | 01 |
| rc-file writers resolve the real `$HOME`; a test could clobber `~/.zshenv` (likely artifact source) | MAJOR | test isolation | 02 |
| Leftover empty `/tmp/.tmp*` Android SDK temp dir | MINOR | temp hygiene | 02 |
| Docs must reflect the auto-PATH-config chain | — | docs | 03 |

## Tasks

| # | Task | Status | Depends On | Sev | Crate | Files Modified (Write) |
|---|---|---|---|---|---|---|
| 01 | [01-auto-configure-path-after-install](tasks/01-auto-configure-path-after-install.md) | ⬜ Todo | — | MAJOR | fdemon-app | `handler/install_wizard/actions.rs`, `message.rs`, `handler/update.rs`, `install_wizard/state.rs` |
| 02 | [02-rc-writer-test-isolation](tasks/02-rc-writer-test-isolation.md) | ⬜ Todo | — | MAJOR | fdemon-daemon | `toolchain/path_config.rs`, `toolchain/android_install.rs` |
| 03 | [03-update-docs](tasks/03-update-docs.md) | ⬜ Todo | 01, 02 | MINOR | docs | `docs/ARCHITECTURE.md` |

**Total Tasks:** 3

## Task Dependency Graph

```
01 auto-config PATH (fdemon-app) ─┐
                                  ├──▶ 03 docs (doc_maintainer)
02 test isolation (fdemon-daemon)─┘
(01 and 02 are file-disjoint → parallel)
```

## File Overlap Analysis

| Task | Files Modified (Write) |
|---|---|
| 01 | `crates/fdemon-app/src/handler/install_wizard/actions.rs`, `crates/fdemon-app/src/message.rs`, `crates/fdemon-app/src/handler/update.rs`, `crates/fdemon-app/src/install_wizard/state.rs` |
| 02 | `crates/fdemon-daemon/src/toolchain/path_config.rs`, `crates/fdemon-daemon/src/toolchain/android_install.rs` |
| 03 | `docs/ARCHITECTURE.md` |

### Overlap Matrix (write-file conflicts)

| Pair | Shared write files | Strategy |
|---|---|---|
| 01 ↔ 02 | none (01 is fdemon-app only; 02 is fdemon-daemon only) | **Parallel (worktree)** |
| 01 ↔ 03 | none | 03 runs after 01 |
| 02 ↔ 03 | none | 03 runs after 02 |

Task 01 only **reads** `actions/mod.rs` and `path_config.rs`; Task 02 only **reads**
`actions/mod.rs`. No write overlap anywhere → 01 and 02 are safe to run
concurrently in isolated worktrees. 03 is docs-only and runs last.

## Suggested Wave Schedule

- **Wave 1 (parallel worktrees):** 01, 02
- **Wave 2:** 03 docs (after 01–02) → `doc_maintainer`

## Success Criteria

- [ ] After the wizard installs Flutter, the correct `<sdk>/bin` is written to the
      shell rc file automatically (no manual PathConfig step); a stale fdemon Flutter
      block is replaced, not duplicated.
- [ ] After the wizard installs Android tools, `ANDROID_HOME` + Android `PATH` are
      written automatically.
- [ ] Preflight still re-runs (and the install is still reported successful) even if
      the auto PathConfig write fails (e.g. unsupported shell); no auto-config loop.
- [ ] The test suite cannot write to a developer's real `~/.zshenv` / `~/.zprofile`
      (regression guard); no leftover empty `/tmp/.tmp*` SDK dirs.
- [ ] `docs/ARCHITECTURE.md` documents the auto-PATH-config chain.
- [ ] `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`,
      `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`
      all pass; no regressions.

## Notes

- Keep blocking rc-file I/O in the `spawn_blocking` executor (`actions/mod.rs`);
  Task 01 changes only emitted messages/actions, not the I/O path.
- Reuse the Phase-7 `run_seq` / `install_task` seq-guard so the auto-started
  PathConfig cannot be clobbered or mis-driven by a stale message.
