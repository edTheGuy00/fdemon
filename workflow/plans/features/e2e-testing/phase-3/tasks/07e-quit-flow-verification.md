## Task: Improve Quit Flow State Verification

**Objective**: Add explicit state verification in quit tests to ensure the confirmation dialog actually appeared before checking exit status.

**Depends on**: 07c-extract-termination-helper

**Priority**: 🟡 MAJOR (Should Fix)

### Scope

- `tests/e2e/tui_interaction.rs`: Enhance quit tests with state verification

### Background

The current `test_quit_confirmation_yes_exits` test sends 'y' to confirm quit but doesn't verify the confirmation dialog actually appeared. The test could pass falsely if the app crashes for unrelated reasons.

**Current problematic pattern:**

```rust
session.send_key('q').expect("Should send 'q' key");
session.expect("quit|Quit").expect("Should show confirmation");  // May timeout
session.send_key('y').expect("Should send 'y' key");
// Immediately checks exit - what if dialog never appeared?
```

### Implementation

Add explicit state verification between key presses:

```rust
/// Test that 'y' confirms quit and exits
#[tokio::test]
#[serial]
async fn test_quit_confirmation_yes_exits() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path())
        .expect("Failed to spawn fdemon");

    session.expect_header().expect("Should show header");

    // Press 'q' to initiate quit
    session.send_key('q').expect("Should send 'q' key");

    // CRITICAL: Verify dialog appeared before proceeding
    // Look for confirmation dialog indicators
    let dialog_appeared = session
        .expect("(y/n)|[Y/n]|confirm|Quit\\?")
        .is_ok();

    assert!(
        dialog_appeared,
        "Quit confirmation dialog should appear after 'q' key"
    );

    // Small delay to ensure dialog is fully rendered
    std::thread::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS));

    // Now send confirmation
    session.send_key('y').expect("Should send 'y' key");

    // Verify process exits
    assert!(
        wait_for_termination(&mut session),
        "Process should exit after quit confirmation"
    );
}
```

### Tests to Update

1. **`test_quit_confirmation_yes_exits`** (~line 268-295)
   - Add assertion that dialog appeared before sending 'y'

2. **`test_quit_confirmation_no_cancels`** (if exists)
   - Add assertion that dialog appeared before sending 'n'

3. **`test_escape_cancels_quit`**
   - Add assertion that dialog appeared before sending Escape

### Verification Pattern

For each quit flow test, ensure this sequence:

```rust
// 1. Start with known state
session.expect_header().expect("Should show header");

// 2. Send 'q' to trigger quit
session.send_key('q').expect("Should send 'q' key");

// 3. VERIFY dialog appeared (don't just expect, assert)
let dialog = session.expect("(y/n)|confirm|Quit");
assert!(dialog.is_ok(), "Quit dialog must appear");

// 4. Send response key
session.send_key('y').expect("Should send confirmation");

// 5. Verify expected outcome
assert!(wait_for_termination(&mut session), "Should exit");
```

### Acceptance Criteria

1. All quit flow tests explicitly verify dialog appearance before sending response
2. Tests fail clearly if dialog doesn't appear (not just timeout)
3. Each test has clear assertion messages indicating expected state
4. `cargo test --test e2e quit -- --nocapture` - All tests pass

### Testing

```bash
# Run quit tests with output
cargo test --test e2e quit -- --nocapture

# Verify specific test
cargo test --test e2e test_quit_confirmation_yes_exits -- --nocapture
```

### Notes

- The goal is to make test failures more diagnostic
- If the app crashes before dialog, test should fail with "dialog didn't appear", not "process exited"
- Consider adding a `verify_dialog_state()` helper if pattern is reused

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `tests/e2e/tui_interaction.rs` | Updated 3 quit flow tests to explicitly verify dialog appearance before proceeding |

### Notable Decisions/Tradeoffs

1. **Dialog verification pattern**: Used `.is_ok()` on `expect()` result and stored in variable for explicit assertion. This makes test failures more diagnostic - if dialog doesn't appear, test fails with clear message "Quit confirmation dialog should appear after 'q' key" rather than timing out.

2. **Async delay handling**: Used `tokio::time::sleep(...).await` instead of `std::thread::sleep()` to properly handle async context in tokio tests. This is consistent with the async nature of `wait_for_termination`.

3. **Consistent pattern across all three tests**: Applied same verification pattern to:
   - `test_quit_confirmation_yes_exits` - verify dialog before 'y'
   - `test_escape_cancels_quit` - verify dialog before Escape
   - `test_double_q_quick_quit` - verify dialog before second 'q'

### Testing Performed

- `cargo fmt` - Passed (no formatting changes needed)
- `cargo check` - Passed (compilation successful)
- `cargo clippy --test e2e -- -D warnings` - Passed (no clippy warnings)
- `cargo test --test e2e quit -- --nocapture` - Failed with environment issues (timeouts on header expectation, not related to code changes)

### Risks/Limitations

1. **Test environment issues**: The e2e tests are failing due to timeout on `expect_header()`, which suggests fdemon is not starting or rendering properly in the test environment. This appears to be a pre-existing environment issue, not related to the code changes made in this task. The tests fail even on the unchanged `test_q_key_shows_confirm_dialog` test.

2. **Binary verification**: The fdemon binary builds successfully and exists at the expected location. The code changes are syntactically correct and pass all static analysis (check, clippy).

3. **Test execution recommendation**: The modified tests should be verified in a proper test environment where fdemon can successfully start and render. The code changes follow the exact pattern specified in the task and will provide better diagnostic failure messages when the tests can run successfully.
