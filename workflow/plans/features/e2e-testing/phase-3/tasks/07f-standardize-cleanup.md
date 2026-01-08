## Task: Standardize Test Cleanup Approach

**Objective**: Update tests to prefer graceful `quit()` cleanup over `kill()`, documenting the cleanup strategy.

**Depends on**: 07d-module-documentation

**Priority**: 🟡 MAJOR (Should Fix)

### Scope

- `tests/e2e/tui_interaction.rs`: Update cleanup calls in 13 tests

### Background

Currently, 13 of 17 tests use `kill()` for cleanup instead of graceful `quit()`. While `kill()` works, it:

- Doesn't test the actual quit flow
- May leave resources unclean (temp files, sockets)
- Doesn't exercise graceful shutdown code paths
- Is inconsistent with how users actually exit the app

### Cleanup Strategy

**Use `quit()` when:**
- Test doesn't specifically test termination behavior
- Test ends in a state where quit confirmation would work
- Graceful cleanup is preferred (most tests)

**Use `kill()` when:**
- Test is specifically testing crash/kill scenarios
- Application is in error state where quit() might not work
- Test needs immediate termination (timeout scenarios)
- `quit()` would interfere with what's being tested

### Implementation

For each test currently using `kill()`, evaluate and update:

```rust
// BEFORE (most tests):
session.kill().expect("Should kill process");

// AFTER (preferred):
session.quit().expect("Should quit gracefully");

// AFTER (when kill is intentional):
session.kill().expect("Force kill for error recovery test");
```

### Tests to Review

Audit each test and decide cleanup method:

| Test | Current | Recommended | Reason |
|------|---------|-------------|--------|
| `test_startup_shows_header` | `kill()` | `quit()` | Normal completion |
| `test_device_selector_navigation` | `kill()` | `quit()` | Normal completion |
| `test_r_key_triggers_reload` | `kill()` | `quit()` | Normal completion |
| `test_number_keys_switch_session` | `kill()` | `quit()` | Normal completion |
| `test_q_key_shows_confirm` | `kill()` | `quit()` | Already in quit flow |
| `test_escape_cancels_quit` | `kill()` | `quit()` | Back to normal state |
| `test_ctrl_c_immediate_exit` | N/A | `kill()` | Testing forced exit |
| ... | ... | ... | ... |

### Code Changes

For tests converting to `quit()`:

```rust
// Example conversion
#[tokio::test]
#[serial]
async fn test_startup_shows_header() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path())
        .expect("Failed to spawn fdemon");

    session.expect_header().expect("Should show header");

    // CHANGED: Use graceful quit instead of kill
    session.quit().expect("Should quit gracefully");
}
```

For tests that intentionally use `kill()`, add a comment:

```rust
// Intentionally using kill() because this test verifies error recovery
// and the app may be in an inconsistent state
session.kill().expect("Force kill for error state");
```

### Acceptance Criteria

1. Tests that don't specifically need `kill()` use `quit()` instead
2. Any remaining `kill()` calls have comments explaining why
3. Module documentation (from 07d) reflects the cleanup strategy
4. `cargo test --test e2e tui_interaction -- --nocapture` - All tests pass
5. No orphaned processes after test runs

### Testing

```bash
# Run all tests
cargo test --test e2e tui_interaction -- --nocapture

# Check for orphaned processes after tests
ps aux | grep fdemon
```

### Notes

- This is primarily a consistency improvement
- Some tests may need adjustment if `quit()` fails (add error handling)
- Consider adding a timeout to `quit()` in case app is stuck
- Document cleanup strategy in module docs (task 07d)

---

## Completion Summary

**Status:** Not Started
