## Task: Create Boolean Toggle E2E Test

**Objective**: Create an E2E test that demonstrates the boolean toggle bug—pressing Enter on a boolean setting should flip true↔false but currently only marks dirty.

**Depends on**: None (can run in parallel with Task 02)

### Scope

- `tests/e2e/settings_page.rs`: Add `test_boolean_toggle_changes_value` test

### Details

Create an E2E test that:
1. Opens fdemon with a test fixture
2. Opens settings page (`,` key)
3. Navigates to a boolean setting (e.g., `auto_reload`)
4. Captures the initial value
5. Presses Enter to toggle
6. Verifies the displayed value changed

The test should **fail** with current code (exposing the bug) and be marked `#[ignore]` with a reason.

```rust
#[tokio::test]
#[serial]
#[ignore = "BUG: Boolean toggle not implemented - see workflow/plans/bugs/boolean-toggle/BUG.md"]
async fn test_boolean_toggle_changes_value() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("spawn");

    // App starts directly in Normal mode (startup flow rework)
    session.expect("Not Connected").expect("startup complete");

    // Open settings - no dialog to dismiss!
    session.send_key(',').expect("send comma");
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Navigate to Project tab (should be default)
    // Find a boolean setting like auto_reload
    // Capture initial value (true or false displayed)

    // Press Enter to toggle
    session.send_key('\r').expect("send enter");
    tokio::time::sleep(Duration::from_millis(200)).await;

    // EXPECTED: Value should flip from true to false (or vice versa)
    // ACTUAL: Value remains unchanged (only dirty flag set)

    // This assertion will FAIL, exposing the bug
    // session.expect("false").expect("value toggled");

    session.quit().expect("quit");
}
```

### Acceptance Criteria

1. Test exists in `tests/e2e/settings_page.rs`
2. Test is marked `#[ignore]` with bug reference
3. Test has clear comments explaining expected vs actual behavior
4. Test would pass if the bug were fixed (verifies correct test logic)

### Testing

Run the test (ignoring ignore attribute) to verify it fails as expected:
```bash
cargo test test_boolean_toggle_changes_value -- --ignored
```

### Notes

- The test documents what **should** happen, not what currently happens
- Do NOT modify the test to pass—the goal is bug detection
- Reference the bug report location in the ignore reason
- Use `serial_test` to avoid test interference

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `tests/e2e/settings_page.rs` | Added `test_boolean_toggle_changes_value()` E2E test (lines 753-857) |

### Notable Decisions/Tradeoffs

1. **Test targets `auto_reload` setting**: Selected this boolean setting because it's readily accessible on the Project tab (second item) and is a user-facing feature that's easy to verify.

2. **Dynamic value detection**: The test captures the initial value and checks whether it's "true" or "false", then verifies the opposite value appears after toggle. This makes the test resilient to config changes.

3. **Detailed assertion messages**: Included extensive diagnostic output in assertions showing the before/after state to make debugging easier when the bug is eventually fixed.

4. **Multiple verification points**: Test verifies both the value change AND the dirty indicator appearance to ensure complete behavior is tested.

### Testing Performed

- `cargo check` - Passed (code compiles)
- `cargo test --lib` - Passed (1329 unit tests, no regression)
- `cargo test test_boolean_toggle_changes_value -- --ignored` - Failed as expected (demonstrates bug)
  - Output confirmed: Boolean value "true" remains "true" after Enter press
  - This is the expected failure showing the bug exists
- `cargo fmt --check` - Passed (code is properly formatted)
- `cargo clippy --test e2e` - No warnings for the new test file

### Risks/Limitations

1. **Test assumes default config values**: The test navigates by position (ArrowDown once) to reach `auto_reload`. If the settings order changes, the test may select a different setting.

2. **Test will fail until bug is fixed**: This is intentional. The test is marked with `#[ignore]` and references the bug report. When the bug is fixed, the ignore attribute should be removed.

3. **String matching for value detection**: Uses simple `contains()` checks for "true"/"false". This may be fragile if the UI representation changes (e.g., checkmarks instead of text).

4. **Timing-sensitive**: E2E tests with PTY interaction are inherently timing-sensitive. Used generous delays to improve reliability across CI and local environments.
