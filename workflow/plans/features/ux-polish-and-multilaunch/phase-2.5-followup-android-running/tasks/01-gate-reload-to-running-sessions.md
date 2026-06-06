## Task: Gate hot-reload to truly-running sessions; never promote a building session to Running

**Agent:** implementor

**Objective:** Stop the launch lifecycle from showing `Running` while the app is
still building. Fix the auto-reload path so it never reloads a session that has
not yet reached `AppPhase::Running`, and harden the reload completion/failure
paths so they can never promote a `Launching`/`Preparing` session to `Running`.

**Depends on:** — (first task; run on the integration branch)

**Estimated Time:** 2–3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/session_manager.rs` — `reloadable_sessions()` gate
- `crates/fdemon-app/src/session/session.rs` — `complete_reload()` guard (+ optional shared restore helper)
- `crates/fdemon-app/src/handler/update.rs` — `SessionReloadFailed` / `SessionRestartFailed` restore guards

**Files Read (Dependencies):**
- `crates/fdemon-core/src/types.rs` — `AppPhase` variants (read-only)

### Background (root cause)

Confirmed from a live Android run log: a file-watcher `AutoReloadTriggered`
(`handler/update.rs:303`) fires ~1 s after launch while Gradle is still building.
`reloadable_sessions()` selects the session because it already has an `app_id`
(assigned at the `app.start` event → `Launching`) and a `cmd_sender`, and is not
`is_busy()`. The reload runs, `SessionReloadCompleted` calls
`Session::complete_reload()`, which unconditionally sets `phase = Running` — so
the still-building app displays as `Running`. The `app.started` event (the only
correct `Running` trigger) never arrived.

The manual `HotReload`/`HotRestart` handlers already gate on `is_running()`
(`handler/update.rs:115,150`); only the auto-reload **selection** path and the
reload **completion/failure** writes are unguarded.

### Details

#### Fix A — `reloadable_sessions()` requires the app to be running (`session_manager.rs:384`)

Current filter keeps a session when: `!is_busy()` AND `app_id.is_some()` AND
`cmd_sender.is_some()`. Add a guard that the session is genuinely running.

- Add an early `return None` when the session is **not** in `AppPhase::Running`.
  Because `is_busy()` already excludes `Reloading`, the simplest correct predicate
  is `handle.session.phase == AppPhase::Running` (equivalently `is_running()` with
  the existing `is_busy()` exclusion). Use whichever reads clearest, but the intent
  is: **only `Running` sessions are auto-reloadable.**
- This makes the auto-reload path consistent with the already-correct manual
  `HotReload`/`HotRestart` gates.
- Effect: an `AutoReloadTriggered` during `Initializing`/`Preparing`/`Launching`
  yields an empty `reloadable` list → `handler/update.rs:303` logs
  "no running sessions" and returns `none()`; no reload is dispatched and the phase
  is untouched. (The in-progress build already compiles the latest source, so
  dropping the reload loses nothing.)

#### Fix B — reload completion/failure must not promote a non-`Reloading` session (`session/session.rs`, `handler/update.rs`)

Defense in depth, so no future/edge reload path can corrupt the launch phase:

1. `Session::complete_reload()` (`session/session.rs:621`): only advance to
   `Running` when the session is currently `Reloading`. Suggested shape:
   ```rust
   pub fn complete_reload(&mut self) {
       // A reload only ever starts from Running (start_reload sets Reloading).
       // If we are not Reloading, this completion is stale/spurious (e.g. a
       // reload that began while the app was still Launching) — do not promote
       // a building/stopped session to Running.
       if self.phase != AppPhase::Reloading {
           self.reload_start_time = None;
           return;
       }
       self.reload_count += 1;
       self.last_reload_time = Some(Local::now());
       self.reload_start_time = None;
       self.phase = AppPhase::Running;
   }
   ```
   (Keep stamping `last_reload_time`/`reload_count` only on a real reload so the
   reload-success flash — `reload_flash_alpha`, Phase 6 — never fires for a
   spurious completion.)

2. `SessionReloadFailed` (`handler/update.rs:233`) and `SessionRestartFailed`
   (`handler/update.rs:290`): they restore `phase = AppPhase::Running` after a
   failed reload. Guard both so they only restore `Running` **from** `Reloading`
   (don't resurrect a `Launching`/`Stopped` session whose reload never legitimately
   started). Prefer a small shared `Session` helper to avoid duplicating the guard,
   e.g.:
   ```rust
   /// Restore Running after a failed reload, but only if we were Reloading.
   pub fn fail_reload(&mut self) {
       self.reload_start_time = None;
       if self.phase == AppPhase::Reloading {
           self.phase = AppPhase::Running;
       }
   }
   ```
   and call it from both failed handlers (replacing the direct `phase = Running` +
   `reload_start_time = None` writes). Keep the existing error log lines.

> Note: with Fix A in place, the auto-reload path can no longer start a reload on a
> `Launching` session, so Fix B is a safety net rather than the primary repair.
> Implement both — Fix B also covers the manual path and any future caller.

### Acceptance Criteria

1. `reloadable_sessions()` returns a session **only** when its phase is `Running`
   (with the existing `app_id` + `cmd_sender` requirements). A session in
   `Initializing`/`Preparing`/`Launching`/`Stopped` — even with `app_id` and
   `cmd_sender` set — is excluded.
2. `AutoReloadTriggered` while the selected session is `Launching` dispatches **no**
   reload action and leaves `phase == Launching` (unit-tested via the handler or
   the manager filter).
3. `Session::complete_reload()` called on a `Launching` (or any non-`Reloading`)
   session does **not** change the phase to `Running`; called on a `Reloading`
   session it sets `Running` and stamps `last_reload_time` (existing behaviour).
4. `SessionReloadFailed` / `SessionRestartFailed` restore `Running` only from
   `Reloading`; on a `Launching` session they leave the phase unchanged.
5. Regression: a normal reload/restart of a `Running` session still goes
   `Running → Reloading → Running`, increments `reload_count`, and triggers the
   reload-success flash.
6. `is_busy()` still matches `Reloading` only (unchanged).

### Testing

```rust
// session_manager.rs tests
// - reloadable_sessions_excludes_launching_session (app_id + cmd_sender set, phase=Launching → empty)
// - reloadable_sessions_includes_running_session (phase=Running → present)
// - reloadable_sessions_excludes_preparing_and_initializing

// session/session.rs tests
// - complete_reload_noop_when_launching (phase stays Launching; reload_count unchanged; last_reload_time None)
// - complete_reload_promotes_only_from_reloading (Reloading → Running, count+1)
// - fail_reload_restores_only_from_reloading (Reloading → Running; Launching → Launching)

// handler/update.rs tests
// - auto_reload_noop_while_launching (AutoReloadTriggered with a Launching session → no action, phase unchanged)
// - session_reload_failed_does_not_resurrect_launching
// - regression: auto_reload_running_session_still_reloads
```

### Notes

- Do **not** change the daemon-event mapping (`app.start` → `Launching`,
  `app.started` → `Running`); it is already correct.
- Do **not** queue dropped reloads for replay-on-`Running` in this task — the
  building app picks up the latest source anyway. (A "reload once running" queue is
  a possible future enhancement, out of scope.)
- Keep all changes within `fdemon-app`; no `fdemon-core`/`fdemon-daemon` edits.

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/session_manager.rs` | Fix A: `reloadable_sessions()` now gates on `AppPhase::Running` as the first filter. Added 3 new tests. |
| `crates/fdemon-app/src/session/session.rs` | Fix B1: `complete_reload()` guarded — no-ops unless `Reloading`. Fix B2: new `fail_reload()` helper. Added 3 new tests. |
| `crates/fdemon-app/src/handler/update.rs` | Fix B3: `SessionReloadFailed` and `SessionRestartFailed` now call `fail_reload()` instead of direct `phase = Running`. Added 4 new tests. |
| `crates/fdemon-app/src/handler/tests.rs` | Updated 5 existing auto-reload tests to call `mark_running()` after `mark_started()` so they exercise truly-running sessions. |

### Notable Decisions/Tradeoffs

1. **Phase check before `is_busy()` in `reloadable_sessions()`**: The `AppPhase::Running` guard is placed first for clarity and because `is_busy()` only matches `Reloading` (already excluded by the Running check). The `is_busy()` call is kept for forward-compatibility with any future busy phases that may be added.

2. **`fail_reload()` shared helper**: Rather than duplicating the guard in both `SessionReloadFailed` and `SessionRestartFailed`, a single `Session::fail_reload()` method encapsulates the logic. This follows the existing `complete_reload()` / `start_reload()` pattern.

3. **Existing test updates**: Five tests in `handler/tests.rs` called `mark_started()` (→ `Launching`) without `mark_running()` (→ `Running`). These tests were implicitly relying on the old unguarded filter. The fix adds `mark_running()` calls so the tests correctly simulate fully-running sessions. The intent (test auto-reload with running sessions) is preserved; only the setup was incomplete.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (2881 + 514 + 1094 + 842 + 1478 + others = all green, 0 failed)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

New tests added:
- `session_manager::tests::reloadable_sessions_excludes_launching_session`
- `session_manager::tests::reloadable_sessions_includes_running_session`
- `session_manager::tests::reloadable_sessions_excludes_preparing_and_initializing`
- `session::tests::complete_reload_noop_when_launching`
- `session::tests::complete_reload_promotes_only_from_reloading`
- `session::tests::fail_reload_restores_only_from_reloading`
- `handler::tests::auto_reload_noop_while_launching`
- `handler::tests::auto_reload_running_session_still_reloads`
- `handler::tests::session_reload_failed_does_not_resurrect_launching`
- `handler::tests::session_restart_failed_does_not_resurrect_launching`

### Risks/Limitations

1. **Dropped auto-reloads during build**: As noted in the task, reloads fired while `Launching` are silently dropped. The building app already picks up the latest source from disk, so this is correct behaviour. No replay queue is implemented (out of scope per the task notes).
