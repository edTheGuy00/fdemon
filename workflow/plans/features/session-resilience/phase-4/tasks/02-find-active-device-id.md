## Task: Add `find_active_by_device_id()` to SessionManager

**Objective**: Add a phase-aware device lookup method to `SessionManager` that only returns sessions with active phases, skipping stopped/quitting ones.

**Depends on**: 01-is-active-method

### Scope

- `crates/fdemon-app/src/session_manager.rs`: Add `find_active_by_device_id()` method

### Details

Add a new method adjacent to the existing `find_by_device_id()` (line 309):

```rust
/// Find an active (non-stopped) session by device_id.
///
/// Unlike `find_by_device_id`, this skips sessions in `Stopped` or `Quitting`
/// phases. Used by the new-session launch guard to allow device reuse after
/// a session exits.
pub fn find_active_by_device_id(&self, device_id: &str) -> Option<SessionId> {
    self.sessions
        .iter()
        .find(|(_, h)| h.session.device_id == device_id && h.session.is_active())
        .map(|(id, _)| *id)
}
```

**Design decision — new method vs modifying `find_by_device_id`:**

A new method is preferred because:
- `find_by_device_id` has exactly one caller (the launch guard), so modifying it in-place would also work. However, keeping the original preserves a general-purpose lookup for potential future use (e.g., "find any session for this device regardless of state").
- The naming makes the intent explicit at the call site.

### Acceptance Criteria

1. `find_active_by_device_id` returns `Some(id)` for sessions with phase `Initializing`, `Running`, or `Reloading`
2. `find_active_by_device_id` returns `None` for sessions with phase `Stopped` or `Quitting`
3. `find_by_device_id` remains unchanged
4. Unit tests cover the phase filtering behavior

### Testing

Add tests in the existing `#[cfg(test)] mod tests` block in `session_manager.rs` (near the existing `test_find_by_device_id` at line 550):

```rust
#[test]
fn test_find_active_by_device_id_skips_stopped_session() {
    let mut manager = SessionManager::new();
    let id = manager.create_session("dev1", "Device 1", "macos", false).unwrap();
    // Session starts as Initializing (active)
    assert!(manager.find_active_by_device_id("dev1").is_some());

    // Mark as stopped
    manager.get_mut(id).unwrap().session.phase = AppPhase::Stopped;
    assert!(manager.find_active_by_device_id("dev1").is_none());
    // Original method still finds it
    assert!(manager.find_by_device_id("dev1").is_some());
}

#[test]
fn test_find_active_by_device_id_finds_running_session() {
    let mut manager = SessionManager::new();
    let id = manager.create_session("dev1", "Device 1", "macos", false).unwrap();
    manager.get_mut(id).unwrap().session.phase = AppPhase::Running;
    assert_eq!(manager.find_active_by_device_id("dev1"), Some(id));
}

#[test]
fn test_find_active_by_device_id_returns_none_for_unknown_device() {
    let manager = SessionManager::new();
    assert!(manager.find_active_by_device_id("nonexistent").is_none());
}
```

### Notes

- The existing `find_by_device_id` is intentionally preserved unchanged.
- This method is used by task 03 to fix the launch guard.

---

## Completion Summary

**Status:** Not Started
