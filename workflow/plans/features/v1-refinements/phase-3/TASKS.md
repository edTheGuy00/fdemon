# Phase 3: Version, GitHub Actions & Install Script - Task Index

## Overview

Surface the app version in the CLI (`--version`) and TUI title bar, create a GitHub Actions release workflow for cross-platform binary builds, and provide a version-aware install/update script.

**Total Tasks:** 5
**Estimated Hours:** 10-14 hours

## Task Dependency Graph

```
┌─────────────────────────┐  ┌─────────────────────────┐  ┌─────────────────────────┐
│  01-version-cli-flag    │  │  02-version-title-bar   │  │  03-cross-config        │
│  (no deps)              │  │  (no deps)              │  │  (no deps)              │
└──────────┬──────────────┘  └─────────────────────────┘  └──────────┬──────────────┘
           │                                                         │
           │                                              ┌──────────┴──────────────┐
           │                                              │  04-release-workflow     │
           │                                              │  (depends on: 03)       │
           │                                              └──────────┬──────────────┘
           │                                                         │
           └──────────────────────┬──────────────────────────────────┘
                                  ▼
                       ┌─────────────────────────┐
                       │  05-install-script       │
                       │  (depends on: 01, 04)    │
                       └─────────────────────────┘
```

**Execution waves:**
- **Wave 1** (parallel): 01-version-cli-flag, 02-version-title-bar, 03-cross-config
- **Wave 2** (after 03): 04-release-workflow
- **Wave 3** (after 01 + 04): 05-install-script

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-version-cli-flag](tasks/01-version-cli-flag.md) | Not Started | - | 0.5-1h | `src/main.rs` |
| 2 | [02-version-title-bar](tasks/02-version-title-bar.md) | Not Started | - | 1-2h | `header.rs` |
| 3 | [03-cross-config](tasks/03-cross-config.md) | Not Started | - | 0.5h | `Cross.toml` |
| 4 | [04-release-workflow](tasks/04-release-workflow.md) | Not Started | 3 | 4-5h | `release.yml` |
| 5 | [05-install-script](tasks/05-install-script.md) | Not Started | 1, 4 | 3-4h | `install.sh` |

## Success Criteria

Phase 3 is complete when:

- [ ] `fdemon --version` prints `fdemon 0.1.0`
- [ ] Title bar displays `Flutter Demon v0.1.0` (version in muted style after bold title)
- [ ] `Cross.toml` exists with pinned aarch64 Linux image
- [ ] `release.yml` workflow is syntactically valid YAML
- [ ] Workflow defines build jobs for all 5 targets (macOS x86_64/aarch64, Linux x86_64/aarch64, Windows x86_64)
- [ ] Release job creates artifacts with `fdemon-v{VERSION}-{TARGET}.{tar.gz|zip}` naming
- [ ] Release job generates SHA256 checksums
- [ ] `install.sh` detects OS and architecture correctly
- [ ] Install script resolves latest version from GitHub API
- [ ] Install script checks installed `fdemon --version` and skips if already up to date
- [ ] Install script shows PATH hint when install dir is not in PATH
- [ ] All existing tests pass + new version/header tests added
- [ ] `cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings` passes

## Notes

- The workspace version `0.1.0` is in `Cargo.toml:7` — all crates inherit via `version.workspace = true`
- `CARGO_PKG_VERSION` is a compile-time env var available in every crate — no `build.rs` needed
- The `fdemon-tui` crate's `CARGO_PKG_VERSION` resolves to the same value as the binary crate since both inherit from workspace
- The title bar is rendered by `MainHeader` widget in `crates/fdemon-tui/src/widgets/header.rs`
- The existing `e2e.yml` workflow runs Docker-based E2E tests — it is unrelated to releases
- Website installation page update is deferred to Phase 4 (task 12-update-installation-page)
