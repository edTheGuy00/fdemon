# Trunk-Based Release Strategy — Task Index

## Overview

Migrate from gitflow (`develop` + `master`) to trunk-based development with a single `main` branch and `workflow_dispatch` releases. See [PLAN.md](PLAN.md) for full design.

**Total Tasks:** 5
**Waves:** 2 (4 parallel + 1 sequential)

## Task Dependency Graph

```
Wave 1 (parallel — no dependencies between them)
┌──────────────────────┐  ┌──────────────────────┐
│ 01-rewrite-release   │  │ 02-simplify-site     │
└──────────────────────┘  └──────────────────────┘
┌──────────────────────┐  ┌──────────────────────┐
│ 03-cliff-bump-config │  │ 04-update-branch-refs│
└──────────┬───────────┘  └──────────┬───────────┘
           │                         │
           └────────┬────────────────┘
                    ▼
Wave 2 (after all Wave 1 tasks are committed)
          ┌──────────────────────┐
          │ 05-branch-migration  │
          │    (manual steps)    │
          └──────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Wave | Files |
|---|------|--------|------------|------|-------|
| 1 | [01-rewrite-release-workflow](tasks/01-rewrite-release-workflow.md) | Done | — | 1 | `.github/workflows/release.yml` |
| 2 | [02-simplify-publish-site](tasks/02-simplify-publish-site.md) | Done | — | 1 | `.github/workflows/publish-site.yml` |
| 3 | [03-cliff-bump-config](tasks/03-cliff-bump-config.md) | Done | — | 1 | `cliff.toml` |
| 4 | [04-update-branch-refs](tasks/04-update-branch-refs.md) | Done | — | 1 | `install.sh`, `README.md`, `website/src/pages/docs/installation.rs` |
| 5 | [05-branch-migration](tasks/05-branch-migration.md) | Done | 1, 2, 3, 4 | 2 | Manual GitHub settings + git commands |

## Success Criteria

Phase 1 is complete when:

- [x] `release.yml` is a self-contained `workflow_dispatch` workflow (version → build → release → website)
- [x] `publish-site.yml` only has `workflow_dispatch` trigger
- [x] `cliff.toml` has `[bump]` section with `initial_tag`
- [x] All `master` branch references in install URLs updated to `main`
- [ ] `develop` branch renamed to `main` on GitHub
- [ ] `master` and all stale feature branches deleted (local + remote)
- [ ] `main` is the default branch on GitHub with branch protection

## Notes

- **No PAT required** — single workflow, no cross-workflow triggers
- **No backmerge needed** — auto-tag creates only a lightweight tag, no commits to `main`
- **Wave 1 tasks are fully parallel** — no file overlaps, can be dispatched simultaneously
- **Wave 2 is manual** — GitHub settings changes + git commands, documented as a runbook
