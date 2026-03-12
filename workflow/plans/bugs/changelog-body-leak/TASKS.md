# Changelog Body Leak Fix - Task Index

## Overview

Fix the website changelog to show clean one-line entries instead of massive paragraphs from squash-merge commit bodies, and apply cosmetic cleanup to branch-name-style subjects and PR number suffixes.

**Total Tasks:** 3
**Estimated Hours:** 1.5-2.5 hours

## Task Dependency Graph

```
┌─────────────────────────────────┐
│  01-strip-message-first-line    │  (critical fix)
└───────────────┬─────────────────┘
                │
        ┌───────┴────────┐
        ▼                ▼
┌──────────────┐  ┌─────────────────────────┐
│ 02-strip-pr  │  │ 03-clean-branch-names   │
│   -number    │  │                         │
└──────────────┘  └─────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-strip-message-first-line](tasks/01-strip-message-first-line.md) | Done | - | 0.5-1h | `website/build.rs` |
| 2 | [02-strip-pr-number](tasks/02-strip-pr-number.md) | Done | 01 | 0.5h | `website/build.rs` |
| 3 | [03-clean-branch-names](tasks/03-clean-branch-names.md) | Done | 01 | 0.5-1h | `website/build.rs` |

## Success Criteria

Complete when:

- [ ] Multi-line commit messages display only the first line
- [ ] Trailing ` (#N)` PR references are stripped from descriptions
- [ ] Branch-name subjects like `Feat/session resilience` render as `Session resilience`
- [ ] All new unit tests pass
- [ ] `cargo check` passes for the website crate
- [ ] No regressions in v0.1.0 changelog entries

## Notes

- Task 01 is the critical fix — tasks 02 and 03 are cosmetic improvements that can run in parallel after 01
- Real commit subjects from post-v0.1.0 for reference:
  - `Feat/session resilience (#3)` — branch-name + PR number
  - `Feat/responsive session dialog (#5)` — same pattern
  - `Fix/release branch protection (#15)` — same pattern
  - `Fix: config.toml watcher paths and auto_start settings (#21)` — title-case conventional-ish
  - `Feature: native platform logs (#20)` — long-form prefix
