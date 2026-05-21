# Version-Check Banner — Task Index

## Overview

Replace the stale `"⚠ Cache-driven auto-launch is now opt-in"` banner with a GitHub-releases version checker that surfaces a one-line `"⬆ New version available: v<X.Y.Z> (current v<A.B.C>)"` banner above the New Session Dialog when a newer fdemon release is on GitHub.

See [PLAN.md](PLAN.md) for the full design rationale.

**Total Tasks:** 6
**Estimated Hours:** 8-12 hours

## Task Dependency Graph

```
Wave 1 (parallel)
┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐
│ 01-version-check │  │ 02-config-key    │  │ 03-banner-       │
│    -module       │  │                  │  │    refactor      │
└────────┬─────────┘  └────────┬─────────┘  └────────┬─────────┘
         │                     │                     │
         └─────────┬───────────┴─────────────────────┘
                   ▼
Wave 2
         ┌──────────────────────┐
         │ 04-spawn-and-wire    │
         └──────────┬───────────┘
                    │
        ┌───────────┴────────────┐
        ▼                        ▼
Wave 3 (parallel)
┌──────────────────────┐  ┌──────────────────────┐
│ 05a-update-config-   │  │ 05b-update-          │
│      docs            │  │     architecture-doc │
│  (implementor)       │  │   (doc_maintainer)   │
└──────────────────────┘  └──────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Agent | Modules |
|---|------|--------|------------|------------|-------|---------|
| 1 | [01-version-check-module](tasks/01-version-check-module.md) | Done | - | 2-3h | implementor | `fdemon-app/version_check.rs`, Cargo.toml |
| 2 | [02-config-key](tasks/02-config-key.md) | Done | - | 0.5-1h | implementor | `fdemon-app/config/types.rs` |
| 3 | [03-banner-refactor](tasks/03-banner-refactor.md) | Done ⚠ | - | 3-4h | implementor | state.rs, message.rs, handler, config/mod.rs, startup.rs, widget, render, headless |
| 4 | [04-spawn-and-wire](tasks/04-spawn-and-wire.md) | Done ⚠ | 1, 2, 3 | 1-2h | implementor | `fdemon-app/spawn.rs`, `fdemon-tui/runner.rs` |
| 5 | [05a-update-config-docs](tasks/05a-update-config-docs.md) | Done | 2, 3 | 0.5-1h | implementor | `docs/CONFIGURATION.md` |
| 6 | [05b-update-architecture-doc](tasks/05b-update-architecture-doc.md) | Done | 1, 4 | 0.5-1h | doc_maintainer | `docs/ARCHITECTURE.md` |

**Concern (Task 03):** `has_cached_last_device` was retained (not deleted) because it has live call sites in `startup.rs` and `headless/runner.rs` for the cache-gate logic. The plan's acceptance criterion 3 grouped it with the nudge-specific symbols incorrectly; the implementor's decision is correct. Plan-author note for future: this function is unrelated to the migration nudge and should not have been listed in the grep-must-be-empty set.

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01-version-check-module | `Cargo.toml` (workspace), `crates/fdemon-app/Cargo.toml`, `crates/fdemon-app/src/version_check.rs` (NEW), `crates/fdemon-app/src/lib.rs` | — |
| 02-config-key | `crates/fdemon-app/src/config/types.rs` | — |
| 03-banner-refactor | `crates/fdemon-app/src/state.rs`, `crates/fdemon-app/src/message.rs`, `crates/fdemon-app/src/handler/update.rs`, `crates/fdemon-app/src/config/mod.rs`, `crates/fdemon-tui/src/startup.rs`, `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs`, `crates/fdemon-tui/src/render/mod.rs`, `src/headless/runner.rs` | — |
| 04-spawn-and-wire | `crates/fdemon-app/src/spawn.rs`, `crates/fdemon-tui/src/runner.rs` | `crates/fdemon-app/src/version_check.rs` (from T01), `crates/fdemon-app/src/message.rs` (from T03), `crates/fdemon-app/src/config/types.rs` (from T02) |
| 05a-update-config-docs | `docs/CONFIGURATION.md` | — |
| 05b-update-architecture-doc | `docs/ARCHITECTURE.md` | `crates/fdemon-app/src/version_check.rs`, `crates/fdemon-app/src/spawn.rs` |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|--------------------|--------------------|
| 01 + 02 | None | Parallel (worktree) |
| 01 + 03 | None | Parallel (worktree) |
| 02 + 03 | None — `config/types.rs` vs `config/mod.rs` are different files | Parallel (worktree) |
| 05a + 05b | None | Parallel (worktree) |

Tasks 04, 05a, 05b are in separate waves from each other (different dependency depths), so no wave-internal overlap analysis is needed beyond 05a + 05b.

## Success Criteria

The feature is complete when:

- [ ] Running `fdemon` shows no "Cache-driven auto-launch" banner at any time
- [ ] When the current crate version is older than the latest GitHub release `tag_name`, the New Session Dialog screen shows a one-line banner: `⬆ New version available: v<latest> (current v<current>)`
- [ ] When the current version is equal or newer, no banner appears
- [ ] Network failure / GitHub 5xx / parse failure during the version check causes no banner and no error UI (silent fail; `tracing::debug!` only)
- [ ] Setting `[behavior] version_check = false` in `.fdemon/config.toml` skips the check entirely (no outbound HTTP)
- [ ] Startup-screen render time is not blocked by the version check (it runs in a `tokio::spawn`'d background task)
- [ ] `cargo test --workspace` passes; net new tests cover: `parse_semver` happy + sad paths, comparator, `startup_notice` default + clear-on-dialog-dismiss, config key default
- [ ] `cargo clippy --workspace` clean
- [ ] All references to `show_migration_banner`, `emit_migration_nudge`, `NudgeMode`, `has_cached_last_device` are deleted (verify with `grep -rn`)

## Notes

- Banner is scoped to the New Session Dialog screen only (matches the old migration banner's scope — once a session is launched, the banner is gone). This was confirmed with the user during planning.
- Headless mode does not spawn the version check (no banner surface there; CI noise concern). 
- HTTP client: `reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "json"] }` — adds ~300 KB and no system-OpenSSL dependency. Confirmed acceptable.
- Branch: `feat/version-check-banner` (already created).
