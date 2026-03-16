# Phase 2 PR Review Followup — Task Index

## Overview

Address 3 issues identified in the Copilot review of PR #23 (`feature/pre-app-custom-sources`). One bug fix (guard logic blocks platform capture), one stale doc comment, and one unused import.

**Total Tasks:** 3
**Review:** [PR #23 Copilot Review](https://github.com/edTheGuy00/fdemon/pull/23)

## Task Dependency Graph

```
┌───────────────────────────────────────────┐
│  01-fix-branch-b-platform-guard           │  BUG — platform capture blocked
│  (independent)                            │
└───────────────────────────────────────────┘

┌───────────────────────────────────────────┐
│  02-doc-and-cleanup                       │  MINOR — stale doc + unused import
│  (independent)                            │
└───────────────────────────────────────────┘
```

Tasks 01 and 02 are independent and can be dispatched in parallel (no file overlap).

## Tasks

| # | Task | Status | Depends On | Severity | Modules |
|---|------|--------|------------|----------|---------|
| 1 | [01-fix-branch-b-platform-guard](tasks/01-fix-branch-b-platform-guard.md) | Done | - | BUG | `handler/session.rs`, `handler/tests.rs` |
| 2 | [02-doc-and-cleanup](tasks/02-doc-and-cleanup.md) | Done | - | MINOR | `handler/mod.rs`, `example/app5/server/server.py` |

## Success Criteria

Followup is complete when:

- [x] Issue 1: Branch B guard no longer blocks platform capture on Android/macOS/iOS with pre-app-only sources
- [x] Issue 2: `SpawnPreAppSources.running_shared_names` doc comment accurately describes data flow
- [x] Issue 3: Unused `import sys` removed from `server.py`
- [x] `cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings` passes
