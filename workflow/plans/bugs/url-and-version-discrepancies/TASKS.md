# URL and Version Discrepancies — Task Index

## Overview

Fix the placeholder `github.com/example/flutter-demon` URL that ships in auto-generated `.fdemon/config.toml` and `.fdemon/launch.toml`, the hardcoded `v0.1.0` release badge on the website home page, and the hardcoded `0.1.0` references on the install docs page. Four independent tasks, no inter-task dependencies — orchestrator can dispatch all four in parallel worktrees.

**Total Tasks:** 4

## Task Dependency Graph

```
┌──────────────────────────────────────┐  ┌──────────────────────────────────────┐
│  01-fix-config-url-generators        │  │  02-update-config-fixtures           │
│  (fdemon-app: settings.rs, launch.rs)│  │  (checked-in .fdemon/config.toml × 3)│
└──────────────────────────────────────┘  └──────────────────────────────────────┘

┌──────────────────────────────────────┐  ┌──────────────────────────────────────┐
│  03-website-home-dynamic-badge       │  │  04-website-install-version-const    │
│  (website/src/pages/home.rs)         │  │  (website/build.rs + installation.rs)│
└──────────────────────────────────────┘  └──────────────────────────────────────┘
```

### Parallelism Waves

| Wave | Tasks | Can Run In Parallel |
|------|-------|---------------------|
| 1 | 01, 02, 03, 04 | Yes (all four in parallel worktrees) |

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-fix-config-url-generators](tasks/01-fix-config-url-generators.md) | Not Started | - | `fdemon-app: config/settings.rs, config/launch.rs` |
| 2 | [02-update-config-fixtures](tasks/02-update-config-fixtures.md) | Not Started | - | `fdemon-tui/.fdemon/config.toml`, `example/app1/.fdemon/config.toml`, `tests/fixtures/simple_app/.fdemon/config.toml` |
| 3 | [03-website-home-dynamic-badge](tasks/03-website-home-dynamic-badge.md) | Not Started | - | `website: src/pages/home.rs` |
| 4 | [04-website-install-version-const](tasks/04-website-install-version-const.md) | Not Started | - | `website: build.rs, src/data.rs, src/pages/docs/installation.rs` |

## File Overlap Analysis

<!-- Orchestrator uses this section to determine isolation strategy per wave -->

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01-fix-config-url-generators | `crates/fdemon-app/src/config/settings.rs`, `crates/fdemon-app/src/config/launch.rs` | `README.md` (canonical URL reference) |
| 02-update-config-fixtures | `crates/fdemon-tui/.fdemon/config.toml`, `example/app1/.fdemon/config.toml`, `tests/fixtures/simple_app/.fdemon/config.toml` | - |
| 03-website-home-dynamic-badge | `website/src/pages/home.rs` | `README.md:11` (canonical badge URL) |
| 04-website-install-version-const | `website/build.rs`, `website/src/data.rs`, `website/src/pages/docs/installation.rs` | `Cargo.toml` (workspace version source) |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|---------------------|
| 01 + 02 | None | Parallel (worktree) |
| 01 + 03 | None | Parallel (worktree) |
| 01 + 04 | None | Parallel (worktree) |
| 02 + 03 | None | Parallel (worktree) |
| 02 + 04 | None | Parallel (worktree) |
| 03 + 04 | None | Parallel (worktree) |

All four tasks can run concurrently with full worktree isolation.

## Success Criteria

The bugfix is complete when:

- [ ] Fresh `fdemon` run in an empty project emits `# See: https://fdemon.dev/docs/configuration` in both `.fdemon/config.toml` and `.fdemon/launch.toml`.
- [ ] `grep -rn "github.com/example" crates/ tests/ example/ website/` returns no hits.
- [ ] A regression test in `crates/fdemon-app/src/config/settings.rs` asserts the new URL is present and the old placeholder is not.
- [ ] Website home page release badge uses the dynamic shields.io GitHub-release endpoint and tracks the latest release without rebuilds.
- [ ] Website installation page renders the current workspace version (currently `0.5.2`) and updates automatically on the next release.
- [ ] `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cd website && trunk build` succeeds without errors.

## Notes

- These are minor user-facing text/UI fixes — no behavioural changes, no architecture impact.
- Task 04 introduces a small `build.rs` extension that reads `../Cargo.toml`. The pattern matches the existing changelog-generation in the same file, so reviewer overhead is minimal.
- No core documentation updates (ARCHITECTURE.md / CODE_STANDARDS.md / DEVELOPMENT.md) needed — these are bug fixes inside existing modules, not architectural changes.
- All tasks ship on the `fix/url-and-version-discrepancies` branch already created off `main`.
