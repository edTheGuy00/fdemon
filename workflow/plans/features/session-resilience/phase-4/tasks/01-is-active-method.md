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

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/session/session.rs` | Added `is_active()` method after `is_busy()` (line 521) |
| `crates/fdemon-app/src/session/tests.rs` | Added 5 unit tests for all `AppPhase` variants, placed adjacent to `test_is_busy` |

### Notable Decisions/Tradeoffs

1. **Placement of `is_active()`**: Added immediately after `is_busy()` as specified in the task, keeping the three lifecycle query methods (`is_running`, `is_busy`, `is_active`) grouped together for discoverability.
2. **`!matches!` macro**: Used the negated `matches!` form to express "not Stopped or Quitting" clearly, consistent with the task specification and the codebase idiom (`matches!` is used in `is_running` and `is_busy`).

### Testing Performed

- `cargo test -p fdemon-app -- is_active` - Passed (5 tests)
- `cargo check -p fdemon-app` - Passed
- `cargo fmt --all` - Passed
- `cargo clippy -p fdemon-app -- -D warnings` - Passed

### Risks/Limitations

1. **None identified**: The method is a simple predicate with no side effects and clearly distinct semantics from `is_running()`. All five `AppPhase` variants are covered by tests.
