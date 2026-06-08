## Task: Document the reload-gating guard in the Session Launch Lifecycle invariant

**Agent:** doc_maintainer

**Objective:** Update `docs/ARCHITECTURE.md` so the documented "Session Launch
Lifecycle" invariant reflects the reload-gating guard added in Task 01. The doc
currently states *"only `app.started` advances the phase to `Running`"* but does
not mention that the auto-reload/reload-completion paths must respect the same
invariant — which is exactly the gap that allowed the premature-`Running` bug.

**Depends on:** 01 (documents the merged behaviour)

**Estimated Time:** ~0.5 hour

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md`

### Details

The "Session Launch Lifecycle" section (around `docs/ARCHITECTURE.md:2095-2120`)
describes the phase flow and the key invariant. Extend it (surgical amendment, no
new top-level section) to record that the `Running` phase is protected on **all**
paths, not just the daemon-event path:

1. Note that **auto-reload only targets `Running` sessions**:
   `SessionManager::reloadable_sessions()` excludes
   `Initializing`/`Preparing`/`Launching` sessions (a session has an `app_id` from
   the `app.start` event while still `Launching`, so `app_id` presence alone is not
   sufficient to be reloadable). The manual `HotReload`/`HotRestart` handlers
   already gate on `is_running()`; the auto-reload selection path now matches.

2. Note that **reload completion/failure never promotes a building session**:
   `Session::complete_reload()` advances to `Running` only from `Reloading`, and
   the `SessionReloadFailed`/`SessionRestartFailed` restores only apply from
   `Reloading`. This keeps the invariant — *"only `app.started` advances an
   initial launch to `Running`"* — true even when a file change fires an
   auto-reload during a long first-compile (e.g. a cold Android Gradle build).

Optionally add one line explaining the platform sensitivity: the guard matters
most on long builds (Android/Gradle) where the `Launching` window is large; on
fast targets the app reaches `Running` before any reload, which is why the bug was
invisible on macOS.

### Acceptance Criteria

1. The "Session Launch Lifecycle" entry states that auto-reload is gated to
   `Running` sessions and that reload completion/failure only restore `Running`
   from `Reloading` — so the `app.started`-only invariant holds during long builds.
2. Wording matches the merged Task 01 implementation (verify against the final
   code, not just this task file).
3. No content-boundary violations (no build/run commands, no how-to prose); this is
   an amendment to the existing architecture description only.
4. The `AppPhase` variants table/line (`docs/ARCHITECTURE.md:485`, `:2282`) remains
   accurate; update only if Task 01 changed a variant's meaning (it does not).

### Notes

- `CONFIGURATION.md` / `KEYBINDINGS.md` are **not** touched — no config keys or
  keybindings change.
- Keep the edit surgical and consistent with the existing section's tone.
