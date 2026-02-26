## Task: Add `is_active()` method to Session

**Objective**: Add a method to `Session` that distinguishes between actively-used sessions (Initializing, Running, Reloading) and terminal sessions (Stopped, Quitting). This provides a semantic complement to the existing `is_running()` method.

**Depends on**: None

### Scope

- `crates/fdemon-app/src/session/session.rs`: Add `is_active()` method

### Details

Add `is_active()` alongside the existing `is_running()` (line 513) and `is_busy()` (line 518):

```rust
/// Check if session is actively in use (not stopped/quitting).
///
/// Unlike `is_running()` which only matches `Running | Reloading`,
/// this also includes `Initializing` — a session that is starting up
/// but hasn't emitted `app.start` yet.
pub fn is_active(&self) -> bool {
    !matches!(self.phase, AppPhase::Stopped | AppPhase::Quitting)
}
```

**Why not just `!= Stopped`?** `Quitting` is an app-level shutdown phase. While it rarely appears on individual sessions, excluding it is semantically correct — a quitting session should not block device reuse.

**Why a new method instead of modifying `is_running()`?** `is_running()` is used throughout the codebase (e.g., `running_sessions()`, `running_count()`, `has_running_sessions()`) with the semantics of "Flutter app is actively running". `Initializing` sessions are NOT running yet — they're waiting for `app.start`. Adding `Initializing` to `is_running()` would change the meaning of all those call sites. `is_active()` has distinct semantics: "this session is occupying the device".

### Acceptance Criteria

1. `Session::is_active()` returns `true` for `Initializing`, `Running`, `Reloading`
2. `Session::is_active()` returns `false` for `Stopped`, `Quitting`
3. `is_running()` behavior is unchanged
4. Unit tests cover all five `AppPhase` variants

### Testing

Add tests adjacent to the existing `is_running` / `is_busy` tests:

```rust
#[test]
fn test_is_active_returns_true_for_initializing() {
    let mut session = Session::new("dev1", "Device 1", "macos", false);
    assert_eq!(session.phase, AppPhase::Initializing);
    assert!(session.is_active());
}

#[test]
fn test_is_active_returns_true_for_running() {
    let mut session = Session::new("dev1", "Device 1", "macos", false);
    session.phase = AppPhase::Running;
    assert!(session.is_active());
}

#[test]
fn test_is_active_returns_true_for_reloading() {
    let mut session = Session::new("dev1", "Device 1", "macos", false);
    session.phase = AppPhase::Reloading;
    assert!(session.is_active());
}

#[test]
fn test_is_active_returns_false_for_stopped() {
    let mut session = Session::new("dev1", "Device 1", "macos", false);
    session.phase = AppPhase::Stopped;
    assert!(!session.is_active());
}

#[test]
fn test_is_active_returns_false_for_quitting() {
    let mut session = Session::new("dev1", "Device 1", "macos", false);
    session.phase = AppPhase::Quitting;
    assert!(!session.is_active());
}
```

### Notes

- This method is the foundation for task 02 (`find_active_by_device_id`).
- Keep `is_running()` unchanged — it has different, well-established semantics throughout the codebase.

---

## Completion Summary

**Status:** Not Started
