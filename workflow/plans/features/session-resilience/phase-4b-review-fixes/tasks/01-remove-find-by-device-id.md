## Task: Remove dead `find_by_device_id` method

**Objective**: Delete the `find_by_device_id` method from `SessionManager` and all associated tests. It has zero production callers after phase 4 swapped the launch guard to `find_active_by_device_id`. Keeping it risks future developers reintroducing the original bug.

**Depends on**: None

### Scope

- `crates/fdemon-app/src/session_manager.rs`: Delete `find_by_device_id` method and its test

### Details

#### 1. Delete the method (lines 309–315)

Remove:

```rust
/// Find session by device_id
pub fn find_by_device_id(&self, device_id: &str) -> Option<SessionId> {
    self.sessions
        .iter()
        .find(|(_, h)| h.session.device_id == device_id)
        .map(|(id, _)| *id)
}
```

#### 2. Delete the test (lines 562–575)

Remove:

```rust
#[test]
fn test_find_by_device_id() {
    let mut manager = SessionManager::new();
    let id1 = manager.create_session(&test_device("device-1", "D1")).unwrap();
    let id2 = manager.create_session(&test_device("device-2", "D2")).unwrap();
    assert_eq!(manager.find_by_device_id("device-1"), Some(id1));
    assert_eq!(manager.find_by_device_id("device-2"), Some(id2));
    assert_eq!(manager.find_by_device_id("device-3"), None);
}
```

#### 3. Remove cross-check in `test_find_active_by_device_id_skips_stopped_session` (line 590)

The test at line 577–591 uses `find_by_device_id` as a cross-check assertion:

```rust
// Original method still finds it
assert!(manager.find_by_device_id("dev1").is_some());
```

Remove that assertion and its comment. The test remains valid — it already asserts that `find_active_by_device_id` returns `None` for stopped sessions (line 588).

### Acceptance Criteria

1. `find_by_device_id` method is deleted from `SessionManager`
2. `test_find_by_device_id` test is deleted
3. Cross-check assertion in `test_find_active_by_device_id_skips_stopped_session` is removed
4. No other code references `find_by_device_id` (search confirms zero callers)
5. `cargo test -p fdemon-app` passes
6. `cargo clippy --workspace -- -D warnings` clean

### Testing

No new tests needed — this is dead code removal. Run full test suite to verify no breakage.

### Notes

- The review recommended adding a doc comment warning. The user explicitly requested deletion instead.
- `find_active_by_device_id` is the correct replacement for all use cases. No phase-blind device lookup is needed anywhere in the codebase.
- Also search for any references in doc comments of other methods that mention `find_by_device_id` (e.g., the `find_active_by_device_id` doc comment says "Unlike `find_by_device_id`..." — that reference should be updated in task 04).
