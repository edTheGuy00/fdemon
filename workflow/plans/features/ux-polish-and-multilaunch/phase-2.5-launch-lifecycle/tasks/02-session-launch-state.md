## Task: Session launch-state helpers, progress field & predicate review

**Objective**: Teach the `Session` model the new lifecycle: `mark_started` now means "launching" (not running), add a dedicated `mark_running()` for the `app.started` transition, add a `current_progress` field with set/clear helpers, and confirm the phase predicates behave correctly for the two new variants. No handler wiring here — task 03 calls these helpers.

**Depends on**: 01-add-launch-phases (the variants must exist)

**Estimated Time**: 1–1.5h

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/session/session.rs`: `mark_started` → `Launching`; add `mark_running()`; add `current_progress: Option<String>` field + `set_progress`/`clear_progress`; review/doc `is_running`/`is_busy`/`is_active`.
- `crates/fdemon-app/src/session/tests.rs`: unit tests for the above.

**Files Read (Dependencies):**
- `crates/fdemon-core/src/types.rs`: `AppPhase::{Preparing, Launching}`.

### Details

**1. `mark_started` now sets `Launching`** (`session.rs:527`). It is called on the `app.start` daemon event, which fires when Flutter *begins* building — not when the app is up. Keep capturing `app_id`; keep `started_at` (used by `session_duration`):

```rust
/// Mark the session as launching: the `app.start` daemon event captured the
/// app id, but the app is still building/starting. Phase flips to `Running`
/// only when `mark_running()` is called from the `app.started` event.
pub fn mark_started(&mut self, app_id: String) {
    self.app_id = Some(app_id);
    self.started_at = Some(Local::now());
    self.phase = AppPhase::Launching;
}
```

**2. Add `mark_running`** — the `app.started` transition:

```rust
/// Mark the session as actually running (the `app.started` daemon event).
/// Clears any in-flight build/readiness progress text.
pub fn mark_running(&mut self) {
    self.phase = AppPhase::Running;
    self.current_progress = None;
}
```

**3. Add `current_progress` field + helpers.** Find the `Session` struct definition and its `Default`/constructor; add:

```rust
/// Latest human-readable launch progress line (Flutter `app.progress`
/// build messages, or pre-app source readiness updates). `None` once
/// the app is running or when there is nothing in flight.
pub current_progress: Option<String>,
```

Initialize it to `None` everywhere `Session` is constructed (constructor + any `Default`/builder). Add:

```rust
/// Set the current launch progress line (shown next to a transient phase label).
pub fn set_progress(&mut self, message: impl Into<String>) {
    self.current_progress = Some(message.into());
}

/// Clear the current launch progress line.
pub fn clear_progress(&mut self) {
    self.current_progress = None;
}
```

**4. Predicate review (mostly doc-only).** With `Launching`/`Preparing` added:
- `is_running()` — **keep** `matches!(self.phase, AppPhase::Running | AppPhase::Reloading)`. The new variants are intentionally *not* running, so reload/restart (gated on `is_running()` in task 03) stay blocked.
- `is_busy()` — **keep** `matches!(self.phase, AppPhase::Reloading)`. Do **not** add the new variants; the bottom metadata bar renders the hardcoded "Reloading" busy label whenever `is_busy` is true, and we don't want the new phases mislabeled.
- `is_active()` — already `!matches!(self.phase, AppPhase::Stopped | AppPhase::Quitting)`, so `Preparing`/`Launching` count as active (correct). Update its doc comment to mention the new variants.

Also ensure `mark_stopped()` / the `app.stop` teardown path leaves `current_progress` cleared (set it to `None` in `mark_stopped`, or clear in task 03's `app.stop` handler — do it in `mark_stopped` here for safety).

### Acceptance Criteria

1. `mark_started(app_id)` sets `phase == AppPhase::Launching`, sets `app_id`, and sets `started_at`.
2. `mark_running()` sets `phase == AppPhase::Running` and clears `current_progress`.
3. `Session` has a `current_progress: Option<String>` field, defaulting to `None` in every constructor; `set_progress`/`clear_progress` behave as named.
4. `is_running()` is `false` for `Preparing` and `Launching`; `is_busy()` is `false` for both; `is_active()` is `true` for both.
5. `mark_stopped()` clears `current_progress`.
6. `cargo test -p fdemon-app` passes (existing + new tests).

### Testing

Add to `session/tests.rs`:

```rust
#[test]
fn mark_started_sets_launching_not_running() { /* phase == Launching, app_id set */ }

#[test]
fn mark_running_sets_running_and_clears_progress() { /* set_progress then mark_running → None */ }

#[test]
fn set_and_clear_progress_roundtrip() { /* set → Some, clear → None */ }

#[test]
fn new_variants_are_active_not_running_not_busy() {
    // phase = Preparing then Launching: is_active==true, is_running==false, is_busy==false
}

#[test]
fn mark_stopped_clears_progress() { /* set_progress then mark_stopped → None */ }
```

Audit existing tests that asserted `mark_started` → `Running` and update them to expect `Launching` (the researcher noted ~14 phase sites in this test file; only those exercising `mark_started`/`app.start` need the `Launching` expectation).

### Notes

- Leaving `started_at` set at `app.start` (Launching) rather than at `Running` preserves existing `session_duration` behavior; note this as a deliberate decision — duration counts from launch, not from first-frame.
- Do not touch handlers, rendering, or `status_icon` here (task 01 did `status_icon`; task 03/04 own handlers/rendering).
