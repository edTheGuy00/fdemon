# Release Workflow Fixes - Task Index

## Overview

Fix two release workflow bugs: Cargo.toml version never gets bumped during releases, and changelog silently drops non-conventional commits.

**Total Tasks:** 3

## Task Dependency Graph

```
┌──────────────────────────────┐     ┌──────────────────────────────┐
│  01-fix-changelog-config     │     │  02-cargo-version-bump       │
└──────────────┬───────────────┘     └──────────────┬───────────────┘
               │                                    │
               └────────────┬───────────────────────┘
                            ▼
               ┌──────────────────────────────┐
               │  03-regenerate-changelog     │
               └──────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-fix-changelog-config](tasks/01-fix-changelog-config.md) | Not Started | - | `cliff.toml` |
| 2 | [02-cargo-version-bump](tasks/02-cargo-version-bump.md) | Not Started | - | `.github/workflows/release.yml` |
| 3 | [03-regenerate-changelog](tasks/03-regenerate-changelog.md) | Not Started | 1, 2 | `CHANGELOG.md` |

## Success Criteria

Release workflow fixes are complete when:

- [ ] `cliff.toml` no longer silently drops non-conventional commits
- [ ] Release workflow updates `Cargo.toml` version before tagging
- [ ] `chore(release)` commits are still excluded from changelog
- [ ] `CHANGELOG.md` reflects all historical commits across all tags
- [ ] `fdemon --version` will report the correct version after next release

## Notes

- Tasks 1 and 2 are independent and can be worked on in parallel
- Task 3 must run after both 1 and 2 are complete
