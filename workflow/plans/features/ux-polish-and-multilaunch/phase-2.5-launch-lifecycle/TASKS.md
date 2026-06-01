# Phase 2.5: Launch Lifecycle Phases — Task Index

## Overview

Today a session flips to `AppPhase::Running` the instant the OS process attaches (`handler/session_lifecycle.rs:21`) and again on the `app.start` daemon event (`session/session.rs:530`) — long before the Flutter app is actually up. The daemon's true "app is running" signal, `app.started` (`DaemonMessage::AppStarted`), drives no phase change today, and build-progress events (`app.progress`, `finished:false`) are dropped (`daemon/protocol.rs:306`). Pre-app native-log sources (`start_before_app` + `ready_check`, e.g. `example/app5`) gate the spawn while the session sits at `Initializing`.

This phase adds two transient phases — **`Preparing`** (pre-app `ready_check` polling) and **`Launching`** (process attached / building / first run) — re-maps the lifecycle so **`app.started` is the sole trigger for `Running`** on initial launch, surfaces live progress text, and reuses Phase 2's shimmer for the new labels. Steady `Running`/`Stopped` and the reload/restart path are unchanged.

**Lifecycle (initial launch):**
```
dialog launch
  ├─(pre-app sources)→ Preparing ──ready_check pass──┐
  └─(no pre-app)─────→ Initializing ────────────────┤
                                                     ▼
                          SessionStarted (process attached) → Launching
                                                     │
                              app.start (capture app_id, stay Launching)
                                                     │
                              app.progress(finished:false) → progress text
                                                     ▼
                                   app.started  →  Running  (clear progress)
```

**Total Tasks:** 5 (4 implementation + 1 doc)
**Estimated Hours:** 4–6h

## Task Dependency Graph

```
┌───────────────────────────────┐
│ 01-add-launch-phases          │  (foundation: enum + display mapping, makes workspace compile)
└───────────────┬───────────────┘
                ▼
┌───────────────────────────────┐
│ 02-session-launch-state       │  (session.rs: transition helpers, current_progress, predicates)
└───────────────┬───────────────┘
                ▼
        ┌───────┴────────┐
        ▼                ▼
┌──────────────────┐ ┌──────────────────────┐
│ 03-wire-launch-  │ │ 04-render-launch-     │   (parallel — disjoint crates/files)
│ transitions      │ │ phases                │
└───────┬──────────┘ └──────────────────────┘
        ▼
┌───────────────────────────────┐
│ 05-doc-launch-lifecycle        │  (doc_maintainer; depends 01–03)
└───────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-add-launch-phases](tasks/01-add-launch-phases.md) | ✅ Done | - | 1h | `core/types.rs`, `tui/theme/styles.rs`, `app/session/session.rs` (status_icon) |
| 2 | [02-session-launch-state](tasks/02-session-launch-state.md) | ✅ Done | 1 | 1–1.5h | `app/session/session.rs`, `app/session/tests.rs` |
| 3 | [03-wire-launch-transitions](tasks/03-wire-launch-transitions.md) | ✅ Done | 2 | 1.5h | `app/handler/session_lifecycle.rs`, `app/handler/session.rs`, `app/handler/new_session/launch_context.rs`, `app/handler/update.rs` |
| 4 | [04-render-launch-phases](tasks/04-render-launch-phases.md) | ✅ Done | 2 | 1.5h | `tui/widgets/log_view/mod.rs`, `tui/widgets/log_view/tests.rs`, `tui/render/mod.rs`, `tui/render/tests.rs` |
| 5 | [05-doc-launch-lifecycle](tasks/05-doc-launch-lifecycle.md) | ✅ Done | 1,2,3 | 0.5h | `docs/ARCHITECTURE.md` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01-add-launch-phases | `crates/fdemon-core/src/types.rs`, `crates/fdemon-tui/src/theme/styles.rs`, `crates/fdemon-app/src/session/session.rs` (only the `status_icon` match) | `crates/fdemon-tui/src/theme/palette.rs` |
| 02-session-launch-state | `crates/fdemon-app/src/session/session.rs`, `crates/fdemon-app/src/session/tests.rs` | `crates/fdemon-core/src/types.rs` |
| 03-wire-launch-transitions | `crates/fdemon-app/src/handler/session_lifecycle.rs`, `crates/fdemon-app/src/handler/session.rs`, `crates/fdemon-app/src/handler/new_session/launch_context.rs`, `crates/fdemon-app/src/handler/update.rs` | `crates/fdemon-app/src/session/session.rs`, `crates/fdemon-core/src/events.rs` |
| 04-render-launch-phases | `crates/fdemon-tui/src/widgets/log_view/mod.rs`, `crates/fdemon-tui/src/widgets/log_view/tests.rs`, `crates/fdemon-tui/src/render/mod.rs`, `crates/fdemon-tui/src/render/tests.rs` | `crates/fdemon-app/src/session/session.rs` (`current_progress`), `crates/fdemon-tui/src/widgets/shimmer.rs`, `crates/fdemon-tui/src/theme/styles.rs` |
| 05-doc-launch-lifecycle | `docs/ARCHITECTURE.md` | all of the above |

### Overlap Matrix

| Task Pair | Same Wave? | Shared Write Files | Isolation Strategy |
|-----------|-----------|-------------------|--------------------|
| 01 + 02 | No (02 depends 01) | `session/session.rs` | Sequential (dependency) — 02 also overlaps `session.rs`, so it must follow 01 regardless |
| 02 + 03 | No (03 depends 02) | None | Sequential (dependency) |
| 02 + 04 | No (04 depends 02) | None | Sequential (dependency) |
| **03 + 04** | **Yes** | **None** (03 = `fdemon-app/handler/*`; 04 = `fdemon-tui/*`) | **Parallel (worktree)** |
| 05 | No (depends 01–03) | None | Sequential (doc, runs last) |

**Waves:**
- **Wave 1** = `01` (single — foundation; makes the workspace compile with the two new variants).
- **Wave 2** = `02` (single — session-state semantics; overlaps `session.rs` with 01 so it follows).
- **Wave 3** = `03` + `04` (**parallel worktrees** — app-side handlers vs. tui-side rendering, zero shared write files). Task 04 compiles/tests against the `current_progress` field and new variants from tasks 01–02; it sets `current_progress` directly in tests and does not need 03's population logic.
- **Wave 4** = `05` (doc_maintainer — depends on the implementation tasks).

## Success Criteria

Phase 2.5 is complete when:

- [ ] `AppPhase` gains `Preparing` and `Launching`; the two exhaustive matches (`theme/styles.rs::phase_indicator`, `session/session.rs::status_icon`) and the `test_phase_indicator_all_phases_covered` array are updated.
- [ ] A freshly launched session shows `Launching` (not `Running`) until the `app.started` daemon event arrives; on initial launch `app.started` is the sole trigger for `Running`.
- [ ] Sessions with `start_before_app` pre-app sources show `Preparing` while `ready_check` polls, before Flutter spawns.
- [ ] `Launching`/`Preparing` labels shimmer (reuse Phase 2) and render in `STATUS_BLUE`; `Running`/`Stopped`/`Reloading` render unchanged.
- [ ] Live build/readiness progress text (`Session::current_progress`) renders next to the label and is cleared on `Running`.
- [ ] Hot reload/restart remain no-ops while `Preparing`/`Launching`; `is_busy` still matches `Reloading` only (no "Reloading" mislabel via the `phase_indicator_busy` path).
- [ ] `cargo test --workspace`, `cargo fmt --all -- --check`, and `cargo clippy --workspace --all-targets -- -D warnings` pass.

## Notes

- **Reload/restart shimmer is out of scope** — that already works (Phase 2) and is intentionally left as-is.
- **`is_busy` must stay `Reloading`-only.** The bottom metadata bar renders `phase_indicator_busy()` (hardcoded "Reloading") whenever `is_busy` is true; if `Launching`/`Preparing` were "busy" they'd be mislabeled "Reloading". Gate reload via `is_running()` (which already excludes the new variants) instead.
- **`is_active()` already includes the new variants** (anything not `Stopped`/`Quitting`), which is the desired behavior — no predicate change needed there beyond the doc comment.
- **Engine events**: the `Launching → Running` transition must not be mistaken for `ReloadCompleted` (that fires only on `Reloading → Running`); verify in task 03, no code change expected.
- **No new config/keybindings/managed-doc surface** beyond the ARCHITECTURE.md note in task 05. A configurable "animations off" toggle remains a PLAN Future Enhancement.
