## Task: Extract Termination Check to Helper Function

**Objective**: Eliminate code duplication by extracting the repeated termination polling loop into a reusable helper function.

**Depends on**: 07b-extract-magic-numbers

**Priority**: 🔴 CRITICAL (Blocking)

### Scope

- `tests/e2e/tui_interaction.rs`: Extract duplicated code to helper function

### Background

Per `docs/CODE_STANDARDS.md:69-72`, avoid unnecessary code duplication. The process termination polling loop appears at least 4 times:

**Duplicated code pattern (found at lines 285-294, 339-353, 370-385, and others):**

```rust
let mut exited = false;
for _ in 0..20 {
    std::thread::sleep(Duration::from_millis(100));
    if let Ok(false) = session.session_mut().is_alive() {
        exited = true;
        break;
    }
}
assert!(exited, "Process should have exited");
```

### Implementation

Add helper function in the test utilities section:

```rust
// ===========================================================================
// Test Helper Functions
// ===========================================================================

/// Wait for the fdemon process to terminate, checking periodically.
///
/// Uses a polling loop to detect when the process exits, with configurable
/// retry count and interval. This is necessary because `quit()` is async
/// and we need to verify the process actually stopped.
///
/// # Arguments
///
/// * `session` - The FdemonSession to check
///
/// # Returns
///
/// `true` if the process terminated within the retry limit, `false` otherwise.
///
/// # Example
///
/// ```rust
/// session.send_key('y').expect("Send confirm");
/// assert!(wait_for_termination(&mut session), "Process should exit after quit confirmation");
/// ```
fn wait_for_termination(session: &mut FdemonSession) -> bool {
    for _ in 0..TERMINATION_CHECK_RETRIES {
        std::thread::sleep(Duration::from_millis(TERMINATION_CHECK_INTERVAL_MS));
        if let Ok(false) = session.session_mut().is_alive() {
            return true;
        }
    }
    false
}
```

Then replace all occurrences:

```rust
// BEFORE:
let mut exited = false;
for _ in 0..20 {
    std::thread::sleep(Duration::from_millis(100));
    if let Ok(false) = session.session_mut().is_alive() {
        exited = true;
        break;
    }
}
assert!(exited, "Process should have exited");

// AFTER:
assert!(
    wait_for_termination(&mut session),
    "Process should have exited"
);
```

### Locations to Update

Search for the pattern and update all occurrences:

1. `test_quit_confirmation_yes_exits` (~line 285-294)
2. `test_escape_cancels_quit` (~line 339-353) - if applicable
3. `test_double_q_quick_quit` (~line 370-385) - if not removed by 07a
4. Any other tests with similar termination check loops

### Acceptance Criteria

1. `wait_for_termination` helper function exists with doc comments
2. Function uses the constants from task 07b
3. Termination polling logic exists in exactly ONE place
4. All tests that previously had inline polling loops now use the helper
5. `cargo fmt` - Passes
6. `cargo check` - No compilation errors
7. `cargo clippy --test e2e -- -D warnings` - No warnings

### Testing

```bash
# Verify single implementation exists
grep -c "fn wait_for_termination" tests/e2e/tui_interaction.rs
# Should return: 1

# Verify no inline polling loops remain
grep -c "session.session_mut().is_alive()" tests/e2e/tui_interaction.rs
# Should return: 1 (only inside the helper)

# Verify tests compile
cargo test --test e2e --no-run
```

### Notes

- This task depends on 07b because it uses the constants defined there
- If task 07a removes `test_double_q_quick_quit`, there's one less location to update
- Consider if the helper should be in `pty_utils.rs` instead (shared across test files)
- The helper simplifies tests and makes timing adjustments easier (single location)

---

## Completion Summary

**Status:** Not Started
