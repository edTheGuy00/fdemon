## Task: Document the launch lifecycle in ARCHITECTURE.md

**Agent: doc_maintainer**

**Objective**: Document the new session launch lifecycle (`Preparing → Launching → Running`) and the `Session::current_progress` field in `docs/ARCHITECTURE.md`, so the daemon-event → phase mapping is discoverable. Strictly within `doc_maintainer` content boundaries.

**Depends on**: 01-add-launch-phases, 02-session-launch-state, 03-wire-launch-transitions

**Estimated Time**: 0.5h

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md`

**Files Read (Dependencies):**
- `crates/fdemon-core/src/types.rs` (`AppPhase`)
- `crates/fdemon-app/src/handler/session.rs`, `handler/session_lifecycle.rs` (transition sites)
- `crates/fdemon-app/src/session/session.rs` (`current_progress`, `mark_running`)

### Details

Read `~/.claude/skills/doc-standards/schemas.md` for content boundary rules before editing.

Update `docs/ARCHITECTURE.md` to reflect:

1. **`AppPhase` variants** — wherever `AppPhase` is described (core types table / Key Types section), note the full set now includes `Preparing` and `Launching`, and that they are transient pre-`Running` states.

2. **Launch lifecycle / data flow** — add a short description (in the Data Flow or Multi-Session section) of the initial-launch phase progression and the daemon events that drive it:
   - `Preparing` — pre-app native-log sources (`start_before_app` + `ready_check`) are polling; Flutter not yet spawned.
   - `Launching` — process attached (`SessionStarted`) and `app.start` received (`app_id` captured); building/first-run.
   - `Running` — set **only** on the `app.started` daemon event (`DaemonMessage::AppStarted`).
   - Note that `app.progress(finished:false)` build messages feed `Session::current_progress` for display.

3. **Session state struct** — if ARCHITECTURE.md enumerates `Session` fields (the Multi-Session Architecture diagram lists session fields), add `current_progress: Option<String>` (latest launch progress line).

### Acceptance Criteria

1. `docs/ARCHITECTURE.md` lists `Preparing` and `Launching` among the `AppPhase` variants.
2. The doc states `app.started` is the trigger for `Running` (not process attach / `app.start`).
3. The pre-app `Preparing` state and `current_progress` are described.
4. No content-boundary violations (no how-to/build steps that belong in DEVELOPMENT.md; no code-standard prescriptions that belong in CODE_STANDARDS.md).
5. Edits are targeted — no unrelated rewrites.

### Notes

- Keep it concise: a few sentences plus list/table updates. Do not duplicate the task-file narrative.
- This is the only managed-doc change in Phase 2.5; there are no new keybindings or config keys to document.
