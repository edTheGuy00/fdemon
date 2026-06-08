# Version-Check Banner Not Appearing — Task Index

## Overview

Fix the three defects that prevent the GitHub version-check banner from ever appearing.
See [BUG.md](BUG.md) for full root-cause analysis and design decisions.

- **#1** poisoned, version-blind 24h disk cache (live, primary)
- **#2** detector self-blindness (latent; harm is cache poisoning, resolved by #1)
- **#3** notice dropped / never rendered outside the New Session Dialog (latent)

**Total Tasks:** 5
**Estimated Hours:** 5–8 hours

## Task Dependency Graph

```
Wave 1 (parallel — disjoint write sets)
┌──────────────────┐  ┌──────────────────────┐  ┌──────────────────────┐
│ 01-cache-rework  │  │ 02-handler-state-    │  │ 03-render-decouple   │
│ (fdemon-app)     │  │    dismiss           │  │ (fdemon-tui)         │
│ version_check.rs │  │ (fdemon-app)         │  │ render + dialog      │
└────────┬─────────┘  └────────┬─────────────┘  └────────┬─────────────┘
         │                     │                         │
         └─────────┬───────────┴─────────────────────────┘
                   ▼
Wave 2 (parallel — docs)
┌──────────────────────┐  ┌──────────────────────┐
│ 04a-config-docs      │  │ 04b-architecture-doc │
│  (implementor)       │  │   (doc_maintainer)   │
└──────────────────────┘  └──────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Agent | Modules |
|---|------|--------|------------|------------|-------|---------|
| 1 | [01-cache-rework](tasks/01-cache-rework.md) | ✅ Done | - | 2-3h | implementor | `fdemon-app/version_check.rs` |
| 2 | [02-handler-state-dismiss](tasks/02-handler-state-dismiss.md) | ✅ Done | - | 1-2h | implementor | `fdemon-app/handler/update.rs`, `fdemon-app/state.rs` |
| 3 | [03-render-decouple](tasks/03-render-decouple.md) | ✅ Done | - | 1.5-2h | implementor | `fdemon-tui/render/mod.rs`, `fdemon-tui/widgets/new_session_dialog/mod.rs` |
| 4 | [04a-config-docs](tasks/04a-config-docs.md) | ✅ Done | 1,2,3 | 0.5h | implementor | `docs/CONFIGURATION.md` |
| 5 | [04b-architecture-doc](tasks/04b-architecture-doc.md) | ✅ Done | 1,2,3 | 0.5h | doc_maintainer | `docs/ARCHITECTURE.md` |

> **04b validation CONCERN (resolved):** ARCHITECTURE.md described a non-existent
> "drops `NewVersionAvailable` on late arrival" handler gate. Corrected post-merge in
> commit `f6b15ab` — the handler stores `startup_notice` unconditionally.

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01-cache-rework | `crates/fdemon-app/src/version_check.rs` | — |
| 02-handler-state-dismiss | `crates/fdemon-app/src/handler/update.rs`, `crates/fdemon-app/src/state.rs` | — |
| 03-render-decouple | `crates/fdemon-tui/src/render/mod.rs`, `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` | `crates/fdemon-app/src/state.rs` (StartupNotice type — read only) |
| 04a-config-docs | `docs/CONFIGURATION.md` | `crates/fdemon-app/src/version_check.rs` |
| 04b-architecture-doc | `docs/ARCHITECTURE.md` | `crates/fdemon-app/src/version_check.rs`, `crates/fdemon-tui/src/render/mod.rs` |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|--------------------|--------------------|
| 01 + 02 | None | Parallel (worktree) |
| 01 + 03 | None | Parallel (worktree) |
| 02 + 03 | None (`state.rs` is written by 02, only *read* by 03) | Parallel (worktree) |
| 04a + 04b | None | Parallel (worktree) |

> Note on 02 + 03: Task 02 writes `state.rs`; Task 03 only *reads* the `StartupNotice` type from
> it (it does not modify it). Read-only overlap is safe for parallel worktrees. Task 03 must not
> edit `state.rs`. If the implementor finds a need to change `StartupNotice`'s definition, that
> change belongs in Task 02 — coordinate rather than editing in both.

## Success Criteria

The fix is complete when:

- [ ] A `0.5.6` binary with latest GitHub `v0.5.7` shows the banner **even after** a `0.5.7`
      build has written the cache (no cross-version poisoning).
- [ ] Deleting the cache and launching still shows the banner.
- [ ] Auto-launch users (no New Session Dialog; `Startup → Loading → Normal`) see the banner on
      the main screen.
- [ ] The banner clears on the first keypress in `Normal`/`Loading`.
- [ ] A binary whose version equals the latest release shows no banner AND writes a cache entry
      that does not suppress an older binary (raw tag stored; `current_version` recorded).
- [ ] An old-format cache file (no `current_version`) is treated as a cache miss, not a crash.
- [ ] `cargo test --workspace` passes; `cargo clippy --workspace` clean; `cargo fmt --all` applied.

## Notes

- Cache path is platform-specific (`dirs::cache_dir()`): Linux `~/.cache/fdemon/`, macOS
  `~/Library/Caches/fdemon/`, Windows `%LOCALAPPDATA%\fdemon\`. Docs must name the right path.
- The live poisoned cache on the Linux dev box was already deleted during planning.
- Suggested branch: `fix/version-check-banner-not-appearing`.
- The `Message::NewVersionAvailable` variant, `spawn_version_check`, config key, and HTTP client
  are all correct and unchanged — do not touch them.
