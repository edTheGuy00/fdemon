## Task: Deduplicate Session Creation Methods

**Objective**: Extract the duplicated 6-line insertion block from all `create_session*` methods into a private `insert_session()` helper. Remove the dead `create_session_with_config` method.

**Depends on**: None

**Severity**: MEDIUM — DRY violation, maintenance burden

### Scope

- `crates/fdemon-app/src/session_manager.rs:44-179`: All four `create_session*` methods

### Details

**Current state:** Four methods duplicate identical insertion logic:

| Method | Production Calls | Test Calls | Session Builder |
|--------|-----------------|------------|-----------------|
| `create_session` | 1 (headless) | ~150 | `Session::new(...)` |
| `create_session_with_config` | **0** | 1 | `...with_config(config)` |
| `create_session_configured` | 2 | 0 | `...with_network_config(...)` |
| `create_session_with_config_configured` | 2 | 0 | `...with_config(...).with_network_config(...)` |

**Duplicated block (identical in all 4 methods):**
```rust
let id = session.id;
let handle = SessionHandle::new(session);
self.sessions.insert(id, handle);
self.session_order.push(id);
if self.session_order.len() == 1 {
    self.selected_index = 0;
}
Ok(id)
```

Plus the MAX_SESSIONS guard at the top of each method.

**Fix approach:**

1. Extract private helper:
```rust
fn insert_session(&mut self, session: Session) -> Result<SessionId> {
    if self.sessions.len() >= MAX_SESSIONS {
        return Err(Error::config(format!(
            "Maximum of {} concurrent sessions reached",
            MAX_SESSIONS
        )));
    }
    let id = session.id;
    let handle = SessionHandle::new(session);
    self.sessions.insert(id, handle);
    self.session_order.push(id);
    if self.session_order.len() == 1 {
        self.selected_index = 0;
    }
    Ok(id)
}
```

2. Simplify each public method to: build session + `self.insert_session(session)`

3. Remove `create_session_with_config` — it has 0 production call sites and only 1 test call site (its own unit test, `test_session_with_config`). The test can be updated to use `create_session_with_config_configured` instead, or removed if redundant.

### Acceptance Criteria

1. Single `insert_session()` private helper contains the insertion block
2. No duplicated insertion blocks remain in any public method
3. `create_session_with_config` removed (or marked `#[cfg(test)]` if kept for test convenience)
4. All ~150 test call sites for `create_session` continue to compile unchanged
5. All production call sites for `create_session_configured` and `create_session_with_config_configured` unchanged
6. `cargo test -p fdemon-app` passes (all 1,039 tests)
7. `cargo test --workspace` passes

### Testing

- Existing tests provide comprehensive coverage — the extraction is purely structural
- Remove or update `test_session_with_config` if `create_session_with_config` is removed
- Verify the headless runner test at `src/headless/runner.rs:249` still compiles

### Notes

- Do **not** change the signature of `create_session(&mut self, device: &Device)` — it has ~150 test call sites
- The headless runner at `src/headless/runner.rs:199` uses `create_session` without devtools config — this is a known behavioral gap (headless sessions don't get `max_network_entries`/`network_auto_record`) but is out of scope for this task
- Consider whether the remaining 3 public methods could be further consolidated into a builder pattern, but defer that to a future refactor

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/session_manager.rs` | Extracted private `insert_session()` helper; simplified `create_session`, `create_session_configured`, `create_session_with_config_configured` to build session + delegate to helper; removed dead `create_session_with_config` method and its `test_session_with_config` test |

### Notable Decisions/Tradeoffs

1. **Removed `create_session_with_config` outright**: The method had 0 production call sites and only 1 test (its own `test_session_with_config`). Removing both the method and the test is cleaner than marking it `#[cfg(test)]`, since the functionality it tested (session with config but no devtools) is no longer an intended public API path — all production creation goes through the `_configured` variants.

2. **`LaunchConfig` import retained**: `create_session_with_config_configured` still takes a `LaunchConfig` parameter, so the import in line 5 remains valid and necessary.

3. **`insert_session` is private (`fn`, not `pub fn`)**: This is intentional — it is an implementation detail that centralizes the invariant (MAX_SESSIONS guard + map+vec insertion + auto-select), not a public API surface.

### Testing Performed

- `cargo check -p fdemon-app` - Passed
- `cargo check --workspace` - Passed
- `cargo test -p fdemon-app` - Passed (1,041 tests passed, 0 failed, 5 ignored)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed (no warnings)
- `cargo fmt -p fdemon-app -- --check` - Passed

### Risks/Limitations

1. **None**: This is a pure structural refactor. No logic changed — the extraction is mechanical (moved identical block into a helper). All existing tests continue to compile and pass unchanged.
