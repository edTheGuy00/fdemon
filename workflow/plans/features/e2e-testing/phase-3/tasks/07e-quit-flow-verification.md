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

**Status:** Not Started
