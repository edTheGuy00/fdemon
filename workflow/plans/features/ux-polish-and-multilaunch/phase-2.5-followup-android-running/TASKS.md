# Phase 2.5 Follow-up: Premature `Running` During Android Builds — Task Index

## Overview

A regression-class bug discovered on Linux/Android: a freshly launched session
shows `AppPhase::Running` **while the app is still building** (Gradle downloading
SDK platforms, compiling), instead of the intended shimmering `Launching` state.
Phase 2.5's documented invariant — *"only `app.started` advances the phase to
`Running`"* (`docs/ARCHITECTURE.md` "Session Launch Lifecycle") — is being
violated by the **auto-reload path**.

### Root cause (confirmed from a live Android run log)

The phase machine itself is correct: `app.start` → `Launching`, and
`app.started` → `Running` is the *only* daemon-event path to `Running`. The
problem is a second, unintended path through hot-reload completion:

Timeline from `./tmp/fdemon-…-2629442.log` (Android, app3):

```
00:15:01.179  Flutter process started (PID)            → Launching
00:15:01.551  Session 1 app started: app_id=143bb13d…  → app.start → still Launching, app_id set
00:15:02.123  Auto-reload triggered for 1 session(s)   → file-watcher fires during build
00:15:02.123  Executing reload for session 1           → reload runs on a BUILDING app
              (SessionReloadCompleted → complete_reload → phase = Running)   ← BUG
00:15:03.008  Auto-reload triggered again
00:15:39      user quits — "app is running" (app.started) NEVER logged
```

Two compounding defects:

1. **`SessionManager::reloadable_sessions()`** (`session_manager.rs:384`) selects
   any session that has an `app_id` and a `cmd_sender` and is not `is_busy()`. But
   `app_id` is assigned at the `app.start` event — i.e. while the session is still
   `Launching`. So the file-watcher auto-reload (`AutoReloadTriggered`,
   `handler/update.rs:303`) reloads a session that is still building. The manual
   `HotReload`/`HotRestart` handlers already gate on `is_running()`
   (`handler/update.rs:115,150`); the auto-reload selection path does **not**, so
   it is the only path that can fire a reload on a non-running session.

2. **`Session::complete_reload()`** (`session/session.rs:621`) unconditionally
   sets `phase = AppPhase::Running`. Completing the spurious reload therefore
   *promotes* the `Launching`/`Reloading` session to `Running` even though
   `app.started` never arrived. The two failure handlers,
   `SessionReloadFailed`/`SessionRestartFailed` (`handler/update.rs:233,290`),
   likewise restore `phase = Running` unconditionally.

**Why it looked fine on macOS:** on a macOS desktop / iOS-sim target the build is
sub-second, so `app.started` → `Running` arrives almost immediately and any
auto-reload happens after the app is genuinely up (where `complete_reload →
Running` is correct). On a cold Android Gradle build the `Launching` window is
30 s–several minutes, and a single file-watcher event in that window (Linux
inotify is prone to emitting one on watch-start) flips the phase to `Running`.
This is a **latent, platform-agnostic bug** exposed by the long build window —
not a Flutter daemon-protocol difference (external research confirms Flutter emits
`app.started` only *after* Gradle build + install on Android).

### Fix strategy (defense in depth)

- **Primary:** gate the auto-reload *selection* (`reloadable_sessions`) on the
  session being genuinely `Running`, bringing it to parity with the already-gated
  manual reload/restart handlers. A session that is `Initializing`/`Preparing`/
  `Launching` is never auto-reloaded — the in-progress build already compiles the
  latest source, so dropping the reload is correct.
- **Safety net:** reload **completion/failure** must never promote a
  non-`Reloading` session to `Running`. `complete_reload()` and the two failed
  handlers only restore `Running` when the session was actually `Reloading`.

Both together guarantee the Phase 2.5 invariant ("only `app.started` →
`Running`") holds even when files change during a long build.

## Defect → Task Map

| Defect | Area | Task |
|--------|------|------|
| Auto-reload selects `Launching` sessions; reload completion promotes them to `Running` | app session/state | 01 |
| ARCHITECTURE "Session Launch Lifecycle" invariant must note the reload-gating guard | docs | 02 |

## Tasks

| # | Task | Status | Depends On | Agent | Est. Hours | Modules |
|---|------|--------|------------|-------|------------|---------|
| 01 | [01-gate-reload-to-running-sessions](tasks/01-gate-reload-to-running-sessions.md) | ✅ Done | - | implementor | 2–3h | `session_manager.rs`, `session/session.rs`, `handler/update.rs` |
| 02 | [02-update-architecture-lifecycle-invariant](tasks/02-update-architecture-lifecycle-invariant.md) | ✅ Done | 01 | doc_maintainer | 0.5h | `docs/ARCHITECTURE.md` |

**Total Tasks:** 2
**Estimated Hours:** 2.5–3.5 hours

## Task Dependency Graph

```
Wave 1:  ┌───────────────────────────────────────────────┐
         │ 01 gate-reload-to-running-sessions            │
         │   fdemon-app: session_manager.rs,             │
         │   session/session.rs, handler/update.rs       │
         └───────────────────────────────────────────────┘
                              │
Wave 2:                       ▼
         ┌───────────────────────────────────────────────┐
         │ 02 docs (doc_maintainer) — after 01           │
         │   docs/ARCHITECTURE.md                        │
         └───────────────────────────────────────────────┘
```

## File Overlap Analysis

### Files Modified (Write) per task

| Task | Files Modified (Write) |
|------|------------------------|
| 01 | `crates/fdemon-app/src/session_manager.rs`, `crates/fdemon-app/src/session/session.rs`, `crates/fdemon-app/src/handler/update.rs` |
| 02 | `docs/ARCHITECTURE.md` |

### Files Read (Dependencies, read-only — no conflict)

- 01 reads: `crates/fdemon-core/src/types.rs` (`AppPhase` variants) — read-only.
- 02 reads: the merged Task 01 source — read-only.

### Overlap Matrix

| Pair | Shared write files | Strategy |
|------|--------------------|----------|
| 01 ↔ 02 | none (app source vs `docs/ARCHITECTURE.md`); 02 also **depends on** 01 | **Sequential by dependency** (02 in Wave 2) |

Each wave contains a single task, so there is no intra-wave parallelism to
arrange; Task 01 runs first (on the working branch, no worktree needed), then
Task 02 documents the merged behaviour.

## Suggested Wave Schedule

- **Wave 1:** 01 (single task, run on the integration branch)
- **Wave 2:** 02 docs (after 01 lands)

## Success Criteria

Phase 2.5 follow-up is complete when:

- [ ] On a cold Android (Gradle) launch, the session stays in the shimmering
      `Launching` state for the entire build and only flips to `Running` when the
      `app.started` daemon event arrives (verified against a fresh run log:
      `"app is running: app_id=…"` precedes the first `Running` display).
      _(logic covered by unit tests; live Android run-log verification still
      pending — manual step.)_
- [x] A file-watcher `AutoReloadTriggered` while a session is
      `Initializing`/`Preparing`/`Launching` is a **no-op** (no reload dispatched,
      phase unchanged) — `reloadable_sessions()` excludes non-`Running` sessions.
- [x] `Session::complete_reload()` only sets `Running` when the session was
      `Reloading`; called on a `Launching` session it leaves the phase unchanged.
- [x] `SessionReloadFailed` / `SessionRestartFailed` only restore `Running` from
      `Reloading` (they don't resurrect a `Launching`/`Stopped` session).
- [x] A normal reload/restart of a genuinely `Running` session is unchanged
      (`Running → Reloading → Running`), including the reload-success flash.
- [x] Unit tests cover: reloadable filtering by phase, no-op auto-reload during
      `Launching`, guarded `complete_reload`, and guarded failed-restore.
- [x] `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`,
      `cargo test --workspace`, and `cargo clippy --workspace --all-targets
      -- -D warnings` all pass.

## Notes

- **No new config keys, no new keybindings.** This is a correctness fix to the
  existing launch-lifecycle/reload state machine.
- The *why* of the spurious watcher event (Linux inotify emitting an event shortly
  after watch-start, even with no real `lib/` change) is **secondary** — the
  phase fix is robust regardless of how often `AutoReloadTriggered` fires, because
  a non-`Running` session is simply never reloaded. If the spurious-event noise is
  worth eliminating on its own, file a separate watcher-debounce task; it is out of
  scope here.
- Keep `is_busy()` matching `Reloading` only (unchanged) so the busy-label path
  doesn't mislabel `Launching`.
