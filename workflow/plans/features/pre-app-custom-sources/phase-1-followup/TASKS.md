# Pre-App Custom Sources — Phase 1 Followup (Review Fixes)

## Overview

Address issues identified in the Phase 1 code review. One critical bug (auto-launch bypass), two major robustness/convention fixes, and five minor code quality improvements.

**Total Tasks:** 8
**Source:** [REVIEW.md](../../reviews/features/pre-app-custom-sources/REVIEW.md)

## Task Dependency Graph

```
Wave 1 (parallel — no deps)
┌───────────────────────────┐  ┌──────────────────────────────┐  ┌───────────────────────────┐
│ 01-auto-launch-gate       │  │ 02-http-buffer-robustness    │  │ 03-ready-check-visibility │
│ CRITICAL: fix bypass      │  │ MAJOR: BufReader status line │  │ MAJOR: pub → pub(super)   │
└───────────────────────────┘  └──────────────────────────────┘  └───────────────────────────┘

Wave 2 (parallel — no deps, independent minor fixes)
┌───────────────────────────┐  ┌──────────────────────────────┐  ┌───────────────────────────┐
│ 04-decompose-spawn-fn     │  │ 05-display-impl-readycheck   │  │ 06-command-check-timeout  │
│ MINOR: extract helper     │  │ MINOR: Display for ReadyCheck│  │ MINOR: align timeout      │
└───────────────────────────┘  └──────────────────────────────┘  └───────────────────────────┘

┌───────────────────────────┐  ┌──────────────────────────────┐
│ 07-url-parser-consistency │  │ 08-tcp-test-stability        │
│ MINOR: remove url crate   │  │ MINOR: dynamic port          │
└───────────────────────────┘  └──────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Severity | Modules |
|---|------|--------|------------|----------|---------|
| 1 | [01-auto-launch-gate](tasks/01-auto-launch-gate.md) | Not Started | - | Critical | `handler/update.rs`, `handler/tests.rs` |
| 2 | [02-http-buffer-robustness](tasks/02-http-buffer-robustness.md) | Not Started | - | Major | `actions/ready_check.rs` |
| 3 | [03-ready-check-visibility](tasks/03-ready-check-visibility.md) | Not Started | - | Major | `actions/mod.rs` |
| 4 | [04-decompose-spawn-fn](tasks/04-decompose-spawn-fn.md) | Not Started | - | Minor | `actions/native_logs.rs` |
| 5 | [05-display-impl-readycheck](tasks/05-display-impl-readycheck.md) | Not Started | - | Minor | `config/types.rs`, `actions/native_logs.rs` |
| 6 | [06-command-check-timeout](tasks/06-command-check-timeout.md) | Not Started | - | Minor | `actions/ready_check.rs` |
| 7 | [07-url-parser-consistency](tasks/07-url-parser-consistency.md) | Not Started | - | Minor | `actions/ready_check.rs`, `config/types.rs`, `Cargo.toml` |
| 8 | [08-tcp-test-stability](tasks/08-tcp-test-stability.md) | Not Started | - | Minor | `actions/ready_check.rs` |

## Success Criteria

Phase 1 followup is complete when:

- [ ] Auto-launch path with `start_before_app` sources emits `SpawnPreAppSources` (not `SpawnSession`)
- [ ] HTTP ready check reads the full status line regardless of TCP segmentation
- [ ] `ready_check` module uses `pub(super)` visibility matching sibling conventions
- [ ] Minor code quality issues addressed (function length, Display impl, timeout consistency, parser consistency, test stability)
- [ ] `cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings`

## Notes

- All 8 tasks are independent and can be dispatched in parallel (Wave 1 for critical/major, Wave 2 for minors)
- Task 01 is the highest priority — it is a correctness bug that violates the feature's core guarantee
- Tasks 02 and 03 are straightforward convention/robustness fixes
- All module paths are relative to `crates/fdemon-app/src/`
