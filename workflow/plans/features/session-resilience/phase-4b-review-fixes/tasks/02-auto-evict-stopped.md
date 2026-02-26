## Task: Auto-evict oldest stopped session when MAX_SESSIONS is reached

**Objective**: Prevent a UX dead-end where all 9 session slots are occupied by stopped sessions, blocking new session creation. When `MAX_SESSIONS` is reached and a new session is requested, auto-evict the oldest stopped session to make room.

**Depends on**: None

### Scope

- `crates/fdemon-app/src/session_manager.rs`: Modify `create_session*` methods to evict stopped sessions before failing

### Background

The `create_session*` methods (lines 44, 79, 117, 152) all check:

```rust
if self.sessions.len() >= MAX_SESSIONS {
    return Err(Error::config(format!(
        "Maximum of {} concurrent sessions reached",
        MAX_SESSIONS
    )));
}
```

This counts ALL sessions including stopped ones. Before phase 4, users were forced to close stopped tabs to reuse a device, so accumulation was less likely. Now that stopped sessions don't block device reuse, users naturally accumulate stopped tabs. After 9 stopped sessions, no new sessions can be created — a UX dead-end.

### Details

#### 1. Add `evict_oldest_stopped` private helper

Add a private method to `SessionManager` that finds and removes the oldest stopped session:

```rust
/// Remove the oldest stopped session to make room for a new one.
/// Returns `true` if a session was evicted, `false` if no stopped sessions exist.
fn evict_oldest_stopped(&mut self) -> bool {
    // Find the first stopped session in session_order (oldest first)
    let stopped_id = self.session_order.iter().find(|&&id| {
        self.sessions
            .get(&id)
            .map_or(false, |h| h.session.phase == AppPhase::Stopped)
    }).copied();

    if let Some(id) = stopped_id {
        self.remove_session(id);
        true
    } else {
        false
    }
}
```

Key decisions:
- Uses `session_order` iteration (oldest first) — consistent with tab ordering
- Reuses existing `remove_session` — handles `session_order` removal and `selected_index` clamping
- Only evicts `Stopped` sessions, not `Quitting` (defensive — `Quitting` is currently unreachable on individual sessions but semantically different)

#### 2. Update the MAX_SESSIONS guard in all four `create_session*` methods

Replace the hard fail with an evict-then-retry pattern:

```rust
if self.sessions.len() >= MAX_SESSIONS {
    if !self.evict_oldest_stopped() {
        return Err(Error::config(format!(
            "Maximum of {} concurrent sessions reached",
            MAX_SESSIONS
        )));
    }
}
```

This must be applied identically to all four methods:
- `create_session` (line 45)
- `create_session_with_config` (line 79)
- `create_session_configured` (line 117)
- `create_session_with_config_configured` (line 152)

Consider extracting the guard into a shared private method to reduce duplication:

```rust
/// Ensure there is room for a new session, evicting the oldest stopped session if needed.
fn ensure_capacity(&mut self) -> Result<()> {
    if self.sessions.len() >= MAX_SESSIONS {
        if !self.evict_oldest_stopped() {
            return Err(Error::config(format!(
                "Maximum of {} concurrent sessions reached",
                MAX_SESSIONS
            )));
        }
    }
    Ok(())
}
```

Then each `create_session*` method starts with `self.ensure_capacity()?;`.

### Acceptance Criteria

1. When `MAX_SESSIONS` is reached and stopped sessions exist, the oldest stopped session is evicted automatically
2. The eviction selects by `session_order` position (oldest first)
3. The error message still appears when all 9 sessions are active (no stopped sessions to evict)
4. The existing `test_max_sessions` test still passes (it creates 9 active sessions, so eviction won't help)
5. New tests cover:
   - 9 stopped sessions → new session creation succeeds (evicts oldest)
   - 8 active + 1 stopped → new session creation succeeds (evicts the stopped one)
   - 9 active → new session creation fails (no stopped sessions to evict)
   - Eviction selects the oldest stopped session (first in `session_order`)
   - `selected_index` remains valid after eviction

### Testing

```rust
#[test]
fn test_create_session_evicts_oldest_stopped_when_full() {
    let mut manager = SessionManager::new();
    // Fill with 9 sessions, mark all stopped
    for i in 0..9 {
        let id = manager.create_session(&test_device(&format!("dev-{i}"), &format!("D{i}"))).unwrap();
        manager.get_mut(id).unwrap().session.phase = AppPhase::Stopped;
    }
    // 10th session should succeed by evicting oldest
    let new_id = manager.create_session(&test_device("dev-new", "New")).unwrap();
    assert!(manager.get(new_id).is_some());
    assert_eq!(manager.sessions.len(), 9); // still at max
    assert!(manager.find_active_by_device_id("dev-0").is_none()); // oldest evicted
}

#[test]
fn test_create_session_fails_when_all_active() {
    let mut manager = SessionManager::new();
    for i in 0..9 {
        let id = manager.create_session(&test_device(&format!("dev-{i}"), &format!("D{i}"))).unwrap();
        manager.get_mut(id).unwrap().session.phase = AppPhase::Running;
    }
    assert!(manager.create_session(&test_device("dev-new", "New")).is_err());
}
```

### Notes

- This is the highest-impact fix from the review — prevents a reachable UX dead-end
- The eviction is invisible to the user (stopped tab disappears) — but this is acceptable because stopped tabs are "dead" UI elements. The user can still see the exit log of the most recently stopped sessions.
- Alternative considered: count only active sessions in the guard. Rejected because it removes the total session cap entirely (unbounded stopped session accumulation would leak memory).
- The `remove_session` call handles `selected_index` clamping, so tab navigation remains valid after eviction

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/session_manager.rs` | Added `evict_oldest_stopped` and `ensure_capacity` private methods; replaced the `MAX_SESSIONS` guard in all four `create_session*` methods with `self.ensure_capacity()?;`; added `AppPhase` import; added 5 new eviction tests |

### Notable Decisions/Tradeoffs

1. **`is_some_and` over `map_or(false, ...)`**: Clippy required the idiomatic `is_some_and` form. The task spec used `map_or` but `is_some_and` is semantically equivalent and passes the `-D warnings` gate.
2. **Collapsed `ensure_capacity` guard**: Clippy flagged the nested `if` as `collapsible_if`; the two conditions were merged with `&&` as suggested. Readability is equivalent.
3. **`AppPhase` import added at crate level**: `fdemon_core::prelude::*` only exports `Error`/`Result`/tracing macros, so `AppPhase` had to be imported explicitly via `use fdemon_core::{prelude::*, AppPhase};`.

### Testing Performed

- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app session_manager` - Passed (26 tests: 21 existing + 5 new)
- `cargo test -p fdemon-app` - Passed (1,156 unit tests + 1 doc-test)
- `cargo clippy --workspace -- -D warnings` - Passed

### Risks/Limitations

1. **Quitting phase not evicted**: `Quitting` sessions are never evicted (by design — they are semantically distinct from `Stopped`). In practice `Quitting` is currently unreachable on individual sessions, so this is a safe conservative choice.
