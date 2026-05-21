# Version-Check Banner Follow-ups — Task Index

## Overview

Address review findings from `workflow/reviews/features/version-check-banner/ACTION_ITEMS.md`. See [PLAN.md](PLAN.md) for design rationale.

**Total Tasks:** 6
**Estimated Hours:** 7–11 hours

## Task Dependency Graph

```
Wave 1 (parallel)
┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────────┐
│ 01-privacy-      │  │ 02-layer-boundary│  │ 03-handler-late- │  │ 04-version-check-    │
│    disclosure    │  │    -exception    │  │    arrival-gate  │  │    hardening         │
└────────┬─────────┘  └────────┬─────────┘  └────────┬─────────┘  └──────────┬───────────┘
         │                     │                     │                       │
         └─────────────────────┴─────────────────────┴───────────────────────┘
                                          │
Wave 2 (parallel)                         ▼
                        ┌──────────────────────┐  ┌──────────────────────┐
                        │ 05-config-defaults-  │  │ 06-update-           │
                        │    and-polish        │  │    architecture-doc  │
                        │  (implementor)       │  │   (doc_maintainer)   │
                        └──────────────────────┘  └──────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Agent | Modules |
|---|------|--------|------------|------------|-------|---------|
| 1 | [01-privacy-disclosure](tasks/01-privacy-disclosure.md) | Done | - | 0.5h | implementor | `docs/CONFIGURATION.md`, `README.md` |
| 2 | [02-layer-boundary-exception](tasks/02-layer-boundary-exception.md) | Done | - | 0.5h | implementor | `docs/REVIEW_FOCUS.md` |
| 3 | [03-handler-late-arrival-gate](tasks/03-handler-late-arrival-gate.md) | Done | - | 1h | implementor | `fdemon-app/handler/update.rs` |
| 4 | [04-version-check-hardening](tasks/04-version-check-hardening.md) | Done (re-validated after follow-up fixes) | - | 4-6h | implementor | `fdemon-app/version_check.rs`, `fdemon-app/spawn.rs`, `fdemon-app/Cargo.toml`, workspace `Cargo.toml`, `fdemon-tui/runner.rs` |
| 5 | [05-config-defaults-and-polish](tasks/05-config-defaults-and-polish.md) | Done | 4 | 1.5-2h | implementor | `fdemon-app/config/types.rs`, `fdemon-app/lib.rs`, `fdemon-app/version_check.rs`, `fdemon-tui/widgets/new_session_dialog/mod.rs`, `src/headless/runner.rs`, `docs/CONFIGURATION.md` |
| 6 | [06-update-architecture-doc](tasks/06-update-architecture-doc.md) | Done | 3, 4 | 0.5h | doc_maintainer | `docs/ARCHITECTURE.md` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01-privacy-disclosure | `docs/CONFIGURATION.md`, `README.md` | — |
| 02-layer-boundary-exception | `docs/REVIEW_FOCUS.md` | — |
| 03-handler-late-arrival-gate | `crates/fdemon-app/src/handler/update.rs` | `crates/fdemon-app/src/state.rs` (read `is_new_session_dialog_visible`) |
| 04-version-check-hardening | `crates/fdemon-app/src/version_check.rs`, `crates/fdemon-app/src/spawn.rs`, `crates/fdemon-app/Cargo.toml`, `Cargo.toml` (workspace), `crates/fdemon-tui/src/runner.rs` | `crates/fdemon-app/src/config/types.rs` (read default timeout for plumbing) |
| 05-config-defaults-and-polish | `crates/fdemon-app/src/config/types.rs`, `crates/fdemon-app/src/lib.rs`, `crates/fdemon-app/src/version_check.rs`, `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs`, `src/headless/runner.rs`, `docs/CONFIGURATION.md` | `crates/fdemon-app/src/spawn.rs` (only if visibility ripple needed) |
| 06-update-architecture-doc | `docs/ARCHITECTURE.md` | `crates/fdemon-app/src/version_check.rs` (post-task-04 state) |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|--------------------|--------------------|
| 01 + 02 | None | Parallel (worktree) |
| 01 + 03 | None | Parallel (worktree) |
| 01 + 04 | None | Parallel (worktree) |
| 02 + 03 | None | Parallel (worktree) |
| 02 + 04 | None | Parallel (worktree) |
| 03 + 04 | None — `handler/update.rs` vs `version_check.rs`+`spawn.rs`+Cargo+`runner.rs` are disjoint | Parallel (worktree) |
| 05 + 06 | None — `docs/ARCHITECTURE.md` is core (doc_maintainer); 05 touches `docs/CONFIGURATION.md` only | Parallel (worktree) |

**Wave 1 → Wave 2 ordering:** task 05 depends on task 04 because it modifies `version_check.rs` (visibility, URL constant) — must run after 04's atomic refactor. Task 05 also touches `docs/CONFIGURATION.md`, which task 01 wrote — this is sequential by dependency depth (Wave 1 finishes before Wave 2 starts), so no conflict.

## Success Criteria

The follow-ups are complete when:

- [ ] `docs/CONFIGURATION.md` and `README.md` contain a Privacy section documenting the GitHub HTTPS call
- [ ] `docs/REVIEW_FOCUS.md` has an Approved Exception entry for `fdemon-app::version_check`
- [ ] Handler arm for `Message::NewVersionAvailable` drops the message when `ui_mode` is not `Startup`/`NewSessionDialog`; new unit test covers both branches
- [ ] On-disk cache at `<dirs::cache_dir()>/fdemon/version_check.json` is read on startup and written after each successful check (24h TTL)
- [ ] HTTP response is capped at 512 KB before JSON parse
- [ ] `parse_semver` tolerates pre-release suffix (`0.6.0-rc.1` → `(0, 6, 0)`) and uses iterator chaining (no `Vec`)
- [ ] `[behavior] version_check_timeout_secs` config key (default `3`) replaces the hardcoded constant
- [ ] `wiremock` integration tests cover the 8-case matrix in PLAN.md Decision 7
- [ ] `pub mod version_check` → `pub(crate)`; same for `check_for_newer_release`
- [ ] Banner copy includes a URL or "see CHANGELOG" hint
- [ ] Banner layout helper deduplicates `render_regions_impl` vs `Widget::render`
- [ ] `behavior_settings_auto_launch_defaults_false` asserts `version_check` and `version_check_timeout_secs`
- [ ] `cargo test --workspace` passes; `cargo clippy --workspace -- -D warnings` clean
- [ ] `docs/ARCHITECTURE.md` reflects the new cache artifact and updated late-arrival gate

## Notes

- Branch: `feat/version-check-banner-followup` (to be created from `feat/version-check-banner` or `main` after the first feature lands)
- All tasks are scoped tight enough that the orchestrator can run Wave 1 fully in parallel worktrees.
- N9 (banner URL hint) and N5 (layout helper) are bundled into task 05 as an opportunistic polish — they aren't worth dedicated tasks.
- ACTION_ITEMS items not addressed by this plan: `N8` (URL drift across README/CONTRIBUTING — low value, no action), additional speculative items beyond NITPICK. See PLAN.md "Open items deferred."
