## Task: Wire the launch lifecycle transitions

**Objective**: Drive the new phases from real events: `Preparing` while pre-app sources poll, `Launching` on process attach + `app.start`, **`Running` only on `app.started`**, and feed `current_progress` from `app.progress` build messages and pre-app readiness updates. Keep hot reload/restart gated until `Running`.

**Depends on**: 02-session-launch-state (uses `mark_running`, `set_progress`, and the new variants)

**Estimated Time**: 1.5h

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/session_lifecycle.rs`: `SessionStarted` → `Launching` instead of `Running`.
- `crates/fdemon-app/src/handler/session.rs`: in `handle_session_message_state`, add `app.started` → `mark_running`, and `app.progress(finished:false)` → `set_progress`.
- `crates/fdemon-app/src/handler/new_session/launch_context.rs`: set `Preparing` when dispatching `SpawnPreAppSources`.
- `crates/fdemon-app/src/handler/update.rs`: feed pre-app readiness text into `set_progress` (and verify the reload/restart gate).

**Files Read (Dependencies):**
- `crates/fdemon-app/src/session/session.rs`: the helpers from task 02.
- `crates/fdemon-core/src/events.rs`: `AppStarted { app_id }`, `AppProgress { app_id, message, finished, .. }`.

### Details

**1. Process attach → `Launching`** (`handler/session_lifecycle.rs:21`). Change the optimistic `Running` assignment:

```rust
// Process pipe is open and building/starting — not yet confirmed up.
handle.session.phase = AppPhase::Launching;
handle.session.started_at = Some(chrono::Local::now());
```

(Keep the `started_at`/pid/pending-error logic.)

**2. `app.started` → `Running`** (`handler/session.rs`, in `handle_session_message_state`, ~line 188). The `app.start` arm already calls `mark_started` (now sets `Launching` after task 02). Add a new arm for `AppStarted`, matching by `app_id` like the other arms:

```rust
// Handle app.started — the app is actually running now.
if let DaemonMessage::AppStarted(app_started) = msg {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        if handle.session.app_id.as_ref() == Some(&app_started.app_id) {
            handle.session.mark_running();
            tracing::info!("Session {} app is running: app_id={}", session_id, app_started.app_id);
        }
    }
}
```

**3. `app.progress(finished:false)` → progress text** (same function). Show the in-flight build line while not yet running:

```rust
if let DaemonMessage::AppProgress(progress) = msg {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        if !handle.session.is_running() {
            match (&progress.message, progress.finished) {
                (Some(m), false) => handle.session.set_progress(m.clone()),
                (_, true) => handle.session.clear_progress(),
                _ => {}
            }
        }
    }
}
```

> Note: `app.progress(finished:false)` is currently dropped in `daemon/protocol.rs::to_log_entry` (it returns `None` for in-progress messages). That filtering only affects the *log buffer*; the structured `DaemonMessage::AppProgress` still reaches `handle_session_message_state`, so no protocol change is required. Leave the log-noise filter as-is.

**4. Pre-app sources → `Preparing`** (`handler/new_session/launch_context.rs`, in `spawn_one`, where `needs_pre_app_spawn` is computed ~line 708). When pre-app spawn is needed, set the freshly-created session to `Preparing`:

```rust
if needs_pre_app_spawn {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.phase = AppPhase::Preparing;
        handle.session.set_progress("Waiting for services…");
    }
    UpdateAction::SpawnPreAppSources { /* … */ }
} else {
    UpdateAction::SpawnSession { /* … */ }
}
```

**5. Pre-app readiness progress** (`handler/update.rs`, the `PreAppSourceProgress` handler ~line 2866 area). Feed its human-readable message into `set_progress` so the `Preparing` label shows "Pre-app sources: N/M ready…". On `PreAppSourcesReady` leave the phase as-is — `SpawnSession` follows and the process attach will move it to `Launching`.

**6. Reload/restart gate (verify, tighten if needed).** Confirm the `HotReload`/`HotRestart`/`StopApp` handlers are gated such that they are no-ops while `Preparing`/`Launching`. They should require `session.is_running()` (which excludes the new variants). If any path keys only off `app_id.is_some()` — note that `app_id` is set during `Launching` (on `app.start`) — add an `is_running()` guard so a reload can't be sent before the app is up.

**7. Engine events (verify, no change expected).** Confirm `engine.rs` emits `PhaseChanged` for the new transitions and that `Launching → Running` is **not** treated as `ReloadCompleted` (that fires only on `Reloading → Running`). Add a regression test if cheap.

### Acceptance Criteria

1. `Message::SessionStarted` sets `phase == AppPhase::Launching` (not `Running`).
2. On initial launch, `phase` becomes `Running` **only** when `DaemonMessage::AppStarted` (matching `app_id`) is processed; before that it is `Launching`.
3. `app.start` keeps the session in `Launching` and captures `app_id` (via `mark_started`).
4. `app.progress(finished:false)` with a message sets `current_progress` while not running; `finished:true` clears it; `app.started`/`mark_running` clears it.
5. A session launched with `start_before_app` pre-app sources is `Preparing` from dialog-confirm until the Flutter process attaches, with readiness progress text.
6. Hot reload/restart are no-ops while `Preparing`/`Launching` (gated on `is_running()`).
7. `Launching → Running` does not emit a spurious `ReloadCompleted` engine event.
8. `cargo test -p fdemon-app` passes.

### Testing

Add to `handler/tests.rs`:
- `session_started_sets_launching` — `SessionStarted` → `Launching`.
- `app_started_event_sets_running` — feed `AppStart` then `AppStarted`; assert `Launching` then `Running`.
- `app_progress_updates_current_progress` — `AppProgress{finished:false, message}` → `current_progress == Some(..)`; `app.started` clears it.
- `pre_app_spawn_sets_preparing` — a launch needing pre-app sources leaves the session `Preparing`.
- `reload_is_noop_while_launching` — `HotReload` during `Launching` produces no reload action.

Audit existing handler tests that assumed `SessionStarted`/`app.start` ⇒ `Running` and update them to expect `Launching`.

### Notes

- This task touches only `fdemon-app/handler/*` — it shares **no** write files with task 04 (tui), so the two run in parallel worktrees.
- Match `AppStarted`/`AppProgress` by `app_id` exactly as the existing `AppStop`/`AppDebugPort` arms do, to avoid cross-session bleed in multi-session setups.
