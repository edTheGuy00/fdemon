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
