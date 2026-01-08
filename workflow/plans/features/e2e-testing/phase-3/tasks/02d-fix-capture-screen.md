## Task: Fix capture_screen() Logic

**Objective**: Fix the `capture_screen()` method to correctly return terminal content, or document the current behavior if intentional.

**Depends on**: 02-pty-test-utilities

### Scope

- `tests/e2e/pty_utils.rs`: Fix or document `capture_screen()` method

### Details

**Current problematic code:**
```rust
pub fn capture_screen(&mut self) -> PtyResult<String> {
    match self.session.expect(Regex(".*")) {
        Ok(found) => {
            let bytes = found.before();  // Returns bytes BEFORE match, likely empty
            Ok(String::from_utf8_lossy(bytes).to_string())
        }
        Err(_) => Ok(String::new()),  // Silent failure on timeout
    }
}
```

**Problems:**
1. `found.before()` returns bytes BEFORE the match, which is likely empty for `.*`
2. Timeout silently returns empty string with no indication of failure
3. May not work as expected for snapshot testing

**Investigation needed:**
1. Check `expectrl` API for correct method to read available output
2. Consider `found.matches()` or `found.get(0)` for matched content
3. Consider `session.try_read()` if available
4. Test actual behavior with a simple fdemon session

**Possible fixes:**

Option A - Use matched content:
```rust
pub fn capture_screen(&mut self) -> PtyResult<String> {
    match self.session.expect(Regex(".+")) {  // Note: .+ requires at least one char
        Ok(found) => {
            let bytes = found.get(0).unwrap_or(&[]);  // Get matched bytes
            Ok(String::from_utf8_lossy(bytes).to_string())
        }
        Err(e) => Err(format!("Failed to capture screen: {}", e).into()),
    }
}
```

Option B - Use try_read for non-blocking read:
```rust
pub fn capture_screen(&mut self) -> PtyResult<String> {
    let mut buffer = Vec::new();
    while let Ok(bytes) = self.session.try_read() {
        if bytes.is_empty() { break; }
        buffer.extend(bytes);
    }
    Ok(String::from_utf8_lossy(&buffer).to_string())
}
```

Option C - Document current behavior if intentional:
```rust
/// Attempts to capture terminal content.
///
/// # Note
/// Returns empty string if no output is available within timeout.
/// This is a best-effort capture and may not include all content.
/// For reliable content verification, use `expect()` with specific patterns.
pub fn capture_screen(&mut self) -> PtyResult<String> {
    // ... existing code with better documentation
}
```

### Acceptance Criteria

1. Method actually returns terminal content, OR
2. Behavior is clearly documented and tests use `expect()` for content verification
3. Timeout no longer silently returns empty string (either error or documented behavior)
4. At least one test verifies capture_screen() returns non-empty content

### Testing

```rust
#[test]
#[ignore]
#[serial]
fn test_capture_screen_returns_content() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).unwrap();
    session.expect_header().unwrap();

    let screen = session.capture_screen().unwrap();
    assert!(!screen.is_empty(), "capture_screen should return content");
    // Could also check for expected content like project name

    session.quit().unwrap();
}
```

### Notes

- May need to consult `expectrl` documentation for correct API usage
- The current implementation may have been intentional as a "drain buffer" operation
- If fixing is complex, documenting limitations and using `expect()` is acceptable

### Review Source

- Logic Reasoning Checker: "capture_screen() Logic Flaw"
- ACTION_ITEMS.md Issue #4

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `/Users/ed/Dev/zabin/flutter-demon/tests/e2e/pty_utils.rs` | Fixed `capture_screen()` method to use `found.get(0)` instead of `found.before()`, added comprehensive documentation, implemented timeout handling with clear behavior |

### Notable Decisions/Tradeoffs

1. **Used Option A (matched content) with enhanced timeout handling**: Changed from `found.before()` to `found.get(0)` to capture the actual matched content. Used `.+` regex instead of `.*` to ensure at least one character is matched.

2. **Implemented graceful timeout handling**: Instead of returning an error on timeout (which would break the best-effort snapshot use case), the method returns an empty string on timeout but documents this behavior clearly. A short 500ms timeout is used to avoid long waits when no output is available.

3. **Added comprehensive documentation**: Included detailed doc comments explaining the method's purpose, return values, and when to use `expect()` instead for reliable content verification. Added a code example showing proper usage.

4. **Updated test to use TUI mode**: The existing test was using headless mode (default for `spawn()`), which outputs JSON events instead of TUI content. Changed the test to spawn in TUI mode (no `--headless` flag) so that actual screen content can be captured and verified.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed
- `cargo clippy -- -D warnings` - Passed
- `cargo test --test e2e test_capture_screen -- --ignored` - Passed (verified method returns non-empty content with project name)
- `cargo test --test e2e -- --ignored` - 5/6 tests passed (1 pre-existing failure in `test_quit` unrelated to this change)

### Risks/Limitations

1. **Empty string on timeout is intentional**: The method returns an empty string if no output is available within 500ms. This is documented as expected behavior for snapshot/debugging use cases. For reliable content verification, users should use `expect()` with specific patterns instead.

2. **ANSI escape codes in captured content**: The captured content includes raw ANSI escape codes from the TUI, making string matching challenging. Tests should either strip ANSI codes or look for basic text content that appears in the output.

3. **Headless mode incompatibility**: This method only works in TUI mode, not headless mode (which outputs JSON events). This is documented in the test and should be noted in any future usage.
