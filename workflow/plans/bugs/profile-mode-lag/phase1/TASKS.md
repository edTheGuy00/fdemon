# Phase 1: Reproduction Setup — Task Index

## Overview

Configure example app3 to reproduce the profile mode lag reported in Issue #25. Add a profile mode launch config with aggressive DevTools polling settings that mirror the reporter's environment, and document the reproduction procedure in TESTING.md.

**Total Tasks:** 2
**Estimated Hours:** 0.5-1 hour

## Task Dependency Graph

```
┌──────────────────────────────┐     ┌──────────────────────────────┐
│  01-add-profile-configs      │     │  02-add-reproduction-test    │
│  (launch.toml + config.toml)│     │  (TESTING.md)                │
└──────────────────────────────┘     └──────────────────────────────┘
         (independent)                        (independent)
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-add-profile-configs](tasks/01-add-profile-configs.md) | Not Started | - | 0.25h | `example/app3/.fdemon/launch.toml`, `example/app3/.fdemon/config.toml` |
| 2 | [02-add-reproduction-test](tasks/02-add-reproduction-test.md) | Not Started | - | 0.25h | `example/TESTING.md` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|----------------------|--------------------------|
| 01-add-profile-configs | `example/app3/.fdemon/launch.toml`, `example/app3/.fdemon/config.toml` | BUG.md (reporter's config reference) |
| 02-add-reproduction-test | `example/TESTING.md` | `example/app3/.fdemon/launch.toml`, `example/app3/.fdemon/config.toml` |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|-------------------|
| 01 + 02 | None | Parallel (worktree) |

## Success Criteria

Phase 1 is complete when:

- [ ] `example/app3/.fdemon/launch.toml` has a "Profile (Issue #25)" config with `mode = "profile"` and `auto_start = true`
- [ ] `example/app3/.fdemon/config.toml` has aggressive DevTools polling matching the reporter's settings (`performance_refresh_ms = 500`, `allocation_profile_interval_ms = 1000`, `network_poll_interval_ms = 1000`)
- [ ] `example/TESTING.md` documents the reproduction procedure for profile mode lag
- [ ] Running `cargo run -- example/app3` auto-launches in profile mode
- [ ] Existing configs (Development, Staging, Production) remain available for comparison testing

## Notes

- App3 was previously used for Issue #18 (multi-config auto_start). The "Staging" config currently has `auto_start = true`. The new profile config should take priority by being listed first with `auto_start = true`, while `auto_start` is removed from Staging to avoid ambiguity. Issue #18 is already fixed and merged (PR #18).
- The reporter's config uses minimum-allowed polling intervals. The code clamps `performance_refresh_ms >= 500ms`, `allocation_profile_interval_ms >= 1000ms`, and `network_poll_interval_ms >= 500ms` (see `actions/performance.rs:28,35` and `actions/network.rs:32`).
- DAP is also enabled in the reporter's config (`dap.enabled = true`). We include this in the reproduction config for completeness, though it may not contribute to the lag.
