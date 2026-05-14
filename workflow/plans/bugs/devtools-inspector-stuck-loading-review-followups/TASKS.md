# DevTools Inspector Review Followups — Task Index

## Overview

Address 4 critical, 4 major, and ~6 minor issues surfaced by the 6-agent review of `fix/devtools-improvements`. See `workflow/reviews/bugs/devtools-inspector-stuck-loading/REVIEW.md` and `ACTION_ITEMS.md` for the source findings.

**Total Tasks:** 12
**Estimated Hours:** 10-14 hours

## Task Dependency Graph

```
Phase 1 (Critical — block merge to main)
01-cache-fallback-isolate-resolution ──┐
                                       │
02-remove-autorehydrate-variant ──> 03-update-architecture-autorehydrate (doc)
                                       │
04-invalidate-cache-on-isolate-exit ───┤
                                       │
05-redact-vm-service-uri-in-logs ──────┘
   (depends on 01 — both write vm_service/client.rs)

Phase 2 (Major — before next release)
06-clear-render-flag-on-hot-restart
07-use-record-fetch-start-at-auto-fetch-sites ──> 08-clamp-readiness-poll-config
                                                     (both write handler/devtools/mod.rs)
09-rename-readiness-poll-config-keys

Phase 3 (Minor — post-release OK)
10-api-hygiene-cleanup
11-code-style-sweep
12-observability-followups
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-cache-fallback-isolate-resolution](tasks/01-cache-fallback-isolate-resolution.md) | Done | — | 0.5-1h | `vm_service/client.rs` |
| 2 | [02-remove-autorehydrate-variant](tasks/02-remove-autorehydrate-variant.md) | Done | — | 0.5-1h | `handler/mod.rs`, `actions/inspector/mod.rs`, `lib.rs` |
| 3 | [03-update-architecture-autorehydrate](tasks/03-update-architecture-autorehydrate.md) | Done | 2 | 0.25h | `docs/ARCHITECTURE.md` |
| 4 | [04-invalidate-cache-on-isolate-exit](tasks/04-invalidate-cache-on-isolate-exit.md) | Done | — | 0.5-1h | `handler/devtools/debug.rs` |
| 5 | [05-redact-vm-service-uri-in-logs](tasks/05-redact-vm-service-uri-in-logs.md) | Done | 1 | 1-2h | `vm_service/client.rs`, `actions/vm_service.rs`, new helper |
| 6 | [06-clear-render-flag-on-hot-restart](tasks/06-clear-render-flag-on-hot-restart.md) | Done | — | 0.5-1h | `handler/update.rs`, `state.rs` |
| 7 | [07-use-record-fetch-start-at-auto-fetch-sites](tasks/07-use-record-fetch-start-at-auto-fetch-sites.md) | Done | — | 0.5h | `handler/devtools/mod.rs` |
| 8 | [08-clamp-readiness-poll-config](tasks/08-clamp-readiness-poll-config.md) | Done | 7 | 1-2h | `handler/devtools/mod.rs` |
| 9 | [09-rename-readiness-poll-config-keys](tasks/09-rename-readiness-poll-config-keys.md) | Done | 8 | 1h | `config/types.rs`, `config/settings.rs`, dispatch sites |
| 10 | [10-api-hygiene-cleanup](tasks/10-api-hygiene-cleanup.md) | Done | — | 1-2h | `lib.rs`, `vm_service/client.rs`, `state.rs` |
| 11 | [11-code-style-sweep](tasks/11-code-style-sweep.md) | Done | — | 1-2h | `actions/inspector/`, `vm_service/client.rs` |
| 12 | [12-observability-followups](tasks/12-observability-followups.md) | Done (concern: warn not asserted via tracing capture — no `tracing-test` dep in workspace; warn path exercised by mock-responder test) | — | 1h | `actions/inspector/mod.rs`, `vm_service/client.rs` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|----------------------|--------------------------|
| 01-cache-fallback-isolate-resolution | `crates/fdemon-daemon/src/vm_service/client.rs` | — |
| 02-remove-autorehydrate-variant | `crates/fdemon-app/src/handler/mod.rs`, `crates/fdemon-app/src/actions/inspector/mod.rs`, `crates/fdemon-app/src/lib.rs` | — |
| 03-update-architecture-autorehydrate | `docs/ARCHITECTURE.md` | tasks/02 |
| 04-invalidate-cache-on-isolate-exit | `crates/fdemon-app/src/handler/devtools/debug.rs` | — |
| 05-redact-vm-service-uri-in-logs | `crates/fdemon-daemon/src/vm_service/client.rs`, `crates/fdemon-daemon/src/vm_service/mod.rs` (new helper), `crates/fdemon-app/src/actions/vm_service.rs` | tasks/01 |
| 06-clear-render-flag-on-hot-restart | `crates/fdemon-app/src/handler/update.rs`, `crates/fdemon-app/src/state.rs` | — |
| 07-use-record-fetch-start-at-auto-fetch-sites | `crates/fdemon-app/src/handler/devtools/mod.rs` | `crates/fdemon-app/src/state.rs` |
| 08-clamp-readiness-poll-config | `crates/fdemon-app/src/handler/devtools/mod.rs` | `crates/fdemon-app/src/config/types.rs` |
| 09-rename-readiness-poll-config-keys | `crates/fdemon-app/src/config/types.rs`, `crates/fdemon-app/src/config/settings.rs`, `crates/fdemon-app/src/handler/devtools/mod.rs`, `crates/fdemon-app/src/handler/mod.rs`, `crates/fdemon-app/src/actions/inspector/mod.rs`, `crates/fdemon-app/src/process.rs` | — |
| 10-api-hygiene-cleanup | `crates/fdemon-app/src/lib.rs`, `crates/fdemon-app/src/handler/mod.rs`, `crates/fdemon-daemon/src/vm_service/client.rs`, `crates/fdemon-app/src/state.rs` | — |
| 11-code-style-sweep | `crates/fdemon-app/src/actions/inspector/mod.rs`, `crates/fdemon-app/src/actions/inspector/widget_tree.rs` | — |
| 12-observability-followups | `crates/fdemon-app/src/actions/inspector/mod.rs`, `crates/fdemon-app/src/actions/inspector/widget_tree.rs`, `crates/fdemon-daemon/src/vm_service/client.rs` | — |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|-------------------|
| 01 + 02 | None | Parallel (worktree) |
| 01 + 04 | None | Parallel (worktree) |
| 01 + 05 | `vm_service/client.rs` | **Sequential (same branch)** — 05 depends on 01 anyway |
| 02 + 04 | None | Parallel (worktree) |
| 02 + 05 | None (post-dependency) | Parallel (worktree) within Phase 1 |
| 04 + 05 | None | Parallel (worktree) |
| 07 + 08 | `handler/devtools/mod.rs` | **Sequential (same branch)** — 08 depends on 07 anyway |
| 06 + 07 | None | Parallel (worktree) |
| 06 + 08 | None | Parallel (worktree) |
| 06 + 09 | None | Parallel (worktree) |
| 07 + 09 | `handler/devtools/mod.rs` | **Sequential (same branch)** — 09 reads dispatch sites |
| 08 + 09 | `handler/devtools/mod.rs` | **Sequential (same branch)** |
| 10 + 11 | None | Parallel (worktree) |
| 10 + 12 | `vm_service/client.rs` | **Sequential (same branch)** |
| 11 + 12 | `actions/inspector/mod.rs`, `widget_tree.rs` | **Sequential (same branch)** |

### Wave Plan

- **Wave 1 (Phase 1, parallel)**: 01, 02, 04 in worktrees (no overlap).
- **Wave 2 (Phase 1, sequential after Wave 1)**: 05 on current branch (writes `vm_service/client.rs` like 01); 03 on current branch (doc, depends on 02 merge).
- **Wave 3 (Phase 2, parallel)**: 06 in worktree (writes `handler/update.rs`, `state.rs`).
- **Wave 4 (Phase 2, sequential)**: 07 → 08 → 09 all touch `handler/devtools/mod.rs` so must run sequentially on the current branch.
- **Wave 5 (Phase 3, sequential)**: 10 → 12 (both touch `client.rs`); 11 → 12 (both touch inspector module). Run 10, 11 in parallel worktrees if no overlap between *them*; then 12 sequentially.

## Success Criteria

Plan is complete when:

- [ ] All Phase 1 tasks merged; `fix/devtools-improvements` clears all review-blocking findings
- [ ] All Phase 2 tasks merged before the next release tag
- [ ] All Phase 3 tasks merged at convenience (no release blocker)
- [ ] All CI quality gates pass (`cargo fmt --all -- --check`, `cargo check`, `cargo test`, `cargo clippy --workspace -- -D warnings`)
- [ ] Unit tests added for: fallback cache write, IsolateExit invalidation, redact helper, hot-restart flag reset

## Notes

- **Pre-existing issues out of scope**: UTF-8 panic in `client.rs:1009`, `let _ =` in `send_close`, and 9-arg `spawn_fetch_widget_tree` refactor are not in this plan. Track separately.
- **Per CLAUDE.md**, custom errors must use the `Error` enum from `fdemon-core/error.rs`.
- **Per docs/CODE_STANDARDS.md**, test names follow `test_<function>_<scenario>_<expected_result>`.
- **AutoRehydrate decision**: user direction is removal (YAGNI). Reintroduce in the PR that adds its first caller.
- **Cache fallback decision**: user direction is to cache the fallback value, matching the existing method docstring.
