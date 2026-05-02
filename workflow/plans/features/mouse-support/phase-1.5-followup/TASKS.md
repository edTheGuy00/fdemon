# Phase 1.5: Foundation Follow-up — Task Index

## Overview

Phase 1 of mouse-support shipped a working foundation but the implementation review (`workflow/reviews/features/mouse-support-phase-1-foundation/REVIEW.md`) surfaced one critical gap (clippy `assertions_on_constants` blocks the project's `-D warnings` quality gate), several quality gaps (missing integration test, undocumented public functions, missing user-facing config doc), one wording correction in the plan itself, and a handful of small hardening items in `terminal.rs`. Phase 1.5 closes all of these so the foundation actually meets its own success criteria, and folds in the cheap `MouseInput::Click` → `MouseInput::Press` rename before Phase 2 consumers exist.

The default `enable_mouse: true` is intentionally preserved per product decision; cross-platform manual smoke testing on Windows + Linux is tracked outside this task index.

**Total Tasks:** 7
**Estimated Hours:** ~4 hours

## Task Dependency Graph

```
                    ┌──────────────────────────────┐
                    │ 01-rename-click-to-press     │
                    │ (input_mouse.rs + event.rs   │
                    │  + handler/mouse.rs tests)   │
                    └──────────────┬───────────────┘
                                   │
       ┌───────────────────┬───────┴────────┬───────────────────┬───────────────────┬─────────────────┐
       ▼                   ▼                ▼                   ▼                   ▼                 ▼
┌─────────────────┐ ┌──────────────┐ ┌─────────────────┐ ┌─────────────────────┐ ┌──────────────┐ ┌──────────────────┐
│ 02-fix-clippy-  │ │ 03-add-      │ │ 04-doc-event-   │ │ 05-document-enable- │ │ 06-correct-  │ │ 07-harden-       │
│ assertions      │ │ update-mouse-│ │ pub-functions   │ │ mouse-config        │ │ no-behavior- │ │ terminal-        │
│ (input_mouse.rs)│ │ integration- │ │ (event.rs)      │ │ (CONFIGURATION.md)  │ │ change-claim │ │ internals        │
│                 │ │ test         │ │                 │ │                     │ │ (PLAN.md +   │ │ (terminal.rs)    │
│                 │ │ (handler/    │ │                 │ │                     │ │  TASKS.md)   │ │                  │
│                 │ │  tests.rs)   │ │                 │ │                     │ │              │ │                  │
└─────────────────┘ └──────────────┘ └─────────────────┘ └─────────────────────┘ └──────────────┘ └──────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Crate / Area |
|---|------|--------|------------|------------|--------------|
| 1 | [01-rename-click-to-press](tasks/01-rename-click-to-press.md) | Not Started | — | 0.5h | `fdemon-app` + `fdemon-tui` |
| 2 | [02-fix-clippy-assertions](tasks/02-fix-clippy-assertions.md) | Not Started | 1 | 0.25h | `fdemon-app` |
| 3 | [03-add-update-mouse-integration-test](tasks/03-add-update-mouse-integration-test.md) | Not Started | 1 | 0.5h | `fdemon-app` |
| 4 | [04-doc-event-pub-functions](tasks/04-doc-event-pub-functions.md) | Not Started | 1 | 0.5h | `fdemon-tui` |
| 5 | [05-document-enable-mouse-config](tasks/05-document-enable-mouse-config.md) | Not Started | — | 0.5h | docs |
| 6 | [06-correct-no-behavior-change-claim](tasks/06-correct-no-behavior-change-claim.md) | Not Started | — | 0.25h | workflow plan |
| 7 | [07-harden-terminal-internals](tasks/07-harden-terminal-internals.md) | Not Started | — | 1.5h | `fdemon-tui` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01-rename-click-to-press | `crates/fdemon-app/src/input_mouse.rs`, `crates/fdemon-tui/src/event.rs`, `crates/fdemon-app/src/handler/mouse.rs` | — |
| 02-fix-clippy-assertions | `crates/fdemon-app/src/input_mouse.rs` | — |
| 03-add-update-mouse-integration-test | `crates/fdemon-app/src/handler/tests.rs` | `crates/fdemon-app/src/input_mouse.rs`, `crates/fdemon-app/src/message.rs`, `crates/fdemon-app/src/state.rs` |
| 04-doc-event-pub-functions | `crates/fdemon-tui/src/event.rs` | — |
| 05-document-enable-mouse-config | `docs/CONFIGURATION.md` | `crates/fdemon-app/src/config/types.rs` (UiSettings.enable_mouse default + serde) |
| 06-correct-no-behavior-change-claim | `workflow/plans/features/mouse-support/phase-1-foundation/TASKS.md`, `workflow/plans/features/mouse-support/PLAN.md` | `workflow/reviews/features/mouse-support-phase-1-foundation/REVIEW.md` (review wording) |
| 07-harden-terminal-internals | `crates/fdemon-tui/src/terminal.rs` | — |

### Overlap Matrix

Wave 1 (no dependencies): 01, 05, 06, 07
Wave 2 (depends on 01): 02, 03, 04

Tasks 05, 06, 07 are wave-1 peers of 01 — they write completely different files (docs, workflow, terminal.rs) so they could in principle run in parallel with 01. **However**, the rename in Task 01 is a sweeping change that future tasks read; running 05/06/07 in parallel with 01 risks the rename worktree being merged after the others, briefly leaving `feat/mouse-support` with `MouseInput::Click` references in code that depended on `Press`. To keep diffs clean and merge order obvious, we run Task 01 alone in Wave 1.

| Task Pair | Wave | Shared Write Files | Isolation Strategy |
|-----------|------|--------------------|---------------------|
| 01 alone | Wave 1 | n/a | **Single task on current branch** (sequential) |
| 02 + 03 | Wave 2 | None | **Parallel (worktree)** |
| 02 + 04 | Wave 2 | None | **Parallel (worktree)** |
| 02 + 05 | Wave 2 | None | **Parallel (worktree)** |
| 02 + 06 | Wave 2 | None | **Parallel (worktree)** |
| 02 + 07 | Wave 2 | None | **Parallel (worktree)** |
| 03 + 04 | Wave 2 | None | **Parallel (worktree)** |
| 03 + 05 | Wave 2 | None | **Parallel (worktree)** |
| 03 + 06 | Wave 2 | None | **Parallel (worktree)** |
| 03 + 07 | Wave 2 | None | **Parallel (worktree)** |
| 04 + 05 | Wave 2 | None | **Parallel (worktree)** |
| 04 + 06 | Wave 2 | None | **Parallel (worktree)** |
| 04 + 07 | Wave 2 | None | **Parallel (worktree)** |
| 05 + 06 | Wave 2 | None | **Parallel (worktree)** |
| 05 + 07 | Wave 2 | None | **Parallel (worktree)** |
| 06 + 07 | Wave 2 | None | **Parallel (worktree)** |

All wave-2 task pairs have zero shared write files — Wave 2 is fully parallelizable.

## Success Criteria

Phase 1.5 is complete when:

- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo check --workspace --all-targets` passes
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes (was failing on Phase 1)
- [ ] `MouseInput` exports `Press` (not `Click`); zero references to `MouseInput::Click` remain in source or tests
- [ ] `crates/fdemon-app/src/handler/tests.rs` contains a test that asserts `update(state, Message::Mouse(...))` returns `UpdateResult::none()` and does not mutate state
- [ ] `crates/fdemon-tui/src/event.rs` has `///` doc comments on `pub fn key_event_to_input` and `pub fn poll`, and a multi-line `//!` module header
- [ ] `docs/CONFIGURATION.md` documents `[ui] enable_mouse`
- [ ] `workflow/plans/features/mouse-support/phase-1-foundation/TASKS.md` overview no longer claims "nothing changes"
- [ ] `crates/fdemon-tui/src/terminal.rs` uses `Release`/`Acquire`/`AcqRel` ordering (not `SeqCst`); `install_panic_hook()` is idempotent; DECSET 1003 trade-off and panic-hook ordering invariants are documented inline

Out of scope (handled separately or deferred):
- Manual cross-platform smoke test on Windows + Linux — tracked as user QA, results to be recorded in `workflow/reviews/features/mouse-support-phase-1-foundation/SMOKE_TEST_RESULTS.md` if/when run
- `KeyModSet.cmd` field, `MouseInput::Scroll.lines` delta — deferred to Phase 2 design where real consumers will surface the right shape
- First-launch discoverability hint for off-switch — out of Phase 1 plumbing scope; revisit if user reports come in
- Insta snapshot version-line filter, settings-count→id assertion — engineering hygiene, not mouse-related; recommend a separate `bugs/test-hygiene/` plan

## Notes

- **No new external dependencies.** All work is in existing files.
- **Default `enable_mouse: true` confirmed.** Per product decision; do not flip in Task 06 — only correct the misleading wording.
- **Click → Press rename rationale.** Crossterm's `MouseEventKind::Down` is what we map to; "Click" implies a debounced down+up which is not what we emit. Renaming is cheap pre-Phase-2 because no consumer uses the variant yet; deferring would create a breaking change once Phase 2 hit-test handlers ship.
- **Why Task 01 runs alone in Wave 1.** Sequential ordering makes the rename diff a single isolated commit on `feat/mouse-support`; other follow-up tasks then build on the renamed code without worktree-merge ordering races.
- **Task 07 bundles three small terminal.rs changes** (atomic ordering relaxation, idempotent panic hook, inline invariant docs) into one task since they all touch the same file and are individually sub-30-min items.
