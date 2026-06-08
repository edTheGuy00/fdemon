# Windows Stale-PATH Re-check — Task Index

## Overview

Fix the confirmed Windows bug where the wizard's re-check (`r`) can't see a
newly-installed PATH tool (git via winget, JDK, …) until fdemon restarts, because
`run_preflight` probes against fdemon's frozen process PATH and never re-reads the
registry. See [`BUG.md`](BUG.md) for the full root-cause analysis.

Approved decisions: **PowerShell read** (no new dependency) and **include the
guided-message wording tweak**.

## Finding → Task Map

| Finding | Sev | Area | Task |
|---|---|---|---|
| Windows re-check uses frozen process PATH; never re-reads registry after a guided install | MAJOR | windows / preflight | 01 |
| Guided text should tell users to press `r` (now works) / open a new terminal | MINOR | UX wording | 01 |
| Docs must describe the Windows preflight PATH-refresh | — | docs | 02 |

## Tasks

| # | Task | Status | Depends On | Sev | Crate | Files Modified (Write) |
|---|------|--------|------------|-----|-------|------------------------|
| 01 | [01-refresh-windows-path-on-preflight](tasks/01-refresh-windows-path-on-preflight.md) | ✅ Done | — | MAJOR | fdemon-daemon, fdemon-app | `toolchain/path_config.rs`, `toolchain/mod.rs`, `install_wizard/state.rs` |
| 02 | [02-update-docs](tasks/02-update-docs.md) | ✅ Done | 01 | MINOR | docs | `docs/ARCHITECTURE.md` |

## Task Dependency Graph

```
01 windows PATH-refresh (fdemon-daemon + fdemon-app) ──▶ 02 docs (doc_maintainer)
```

## File Overlap Analysis

| Task | Files Modified (Write) |
|------|------------------------|
| 01 | `crates/fdemon-daemon/src/toolchain/path_config.rs`, `crates/fdemon-daemon/src/toolchain/mod.rs`, `crates/fdemon-app/src/install_wizard/state.rs` |
| 02 | `docs/ARCHITECTURE.md` |

### Overlap Matrix (write-file conflicts)

| Pair | Shared write files | Strategy |
|------|--------------------|----------|
| 01 ↔ 02 | none | **Sequential** — 02 depends on 01 (docs after impl) |

Only two tasks, in a dependency chain → no parallelism. 01 runs on the current
branch (single task), then 02 (docs) after it.

## Suggested Wave Schedule

- **Wave 1:** 01 (single task, current branch)
- **Wave 2:** 02 docs (after 01) → `doc_maintainer`

## Success Criteria

- [ ] On Windows, after a guided prerequisite install (git/JDK), pressing `r`
      re-detects the tool **without restarting fdemon**.
- [ ] Non-Windows behaviour unchanged (no refresh, no subprocess); initial preflight
      is a near-no-op.
- [ ] Prerequisites guided wording clarifies press-`r` / new-terminal.
- [ ] `docs/ARCHITECTURE.md` documents the Windows preflight PATH-refresh.
- [ ] `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`,
      `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`
      all pass; **E2E verified in the Windows VM** (`tests/docker/windows/`).

## Notes

- The fix is Windows-only (`#[cfg(windows)]`); Linux/macOS guided installs land in
  already-on-PATH dirs, so no refresh is needed.
- `std::env::set_var("PATH", …)` is process-global / `unsafe` in Rust 2024 — apply
  once up-front in `run_preflight`.
- Authoritative verification is the real Windows 11 VM bed already built in
  `tests/docker/windows/` (rebuild `fdemon.exe`, re-stage, walk the re-check).
