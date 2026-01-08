## Task: Document Public Terminal Field

**Objective**: Add documentation explaining why the `terminal` field in `TestTerminal` is public and how to use it properly.

**Depends on**: None

**Priority**: Critical (required before merge)

### Scope

- `src/tui/test_utils.rs`: Line 39 (`pub terminal` field)

### Details

The `TestTerminal` wrapper exposes a public `terminal` field for cases where direct access is needed (e.g., calling `terminal.draw()` for full-frame rendering).

**Current (undocumented):**
```rust
pub struct TestTerminal {
    pub terminal: Terminal<TestBackend>,
}
```

**Add documentation:**
```rust
/// Test utility wrapper around ratatui's TestBackend terminal.
///
/// Provides ergonomic methods for widget testing while maintaining
/// access to the underlying terminal for advanced use cases.
///
/// # Usage
///
/// For simple widget testing, use the wrapper methods:
/// ```ignore
/// let mut term = TestTerminal::new();
/// term.render_widget(my_widget, term.area());
/// assert!(term.buffer_contains("expected text"));
/// ```
///
/// For full-frame rendering (like `tui::view`), use the terminal directly:
/// ```ignore
/// let mut term = TestTerminal::new();
/// term.terminal.draw(|frame| view(frame, &state))?;
/// ```
pub struct TestTerminal {
    /// The underlying ratatui terminal with TestBackend.
    ///
    /// This field is public to allow direct access for:
    /// - Full-frame rendering with `terminal.draw(|frame| ...)`
    /// - Advanced terminal operations not covered by wrapper methods
    ///
    /// Prefer using wrapper methods (`render_widget`, `buffer_contains`, etc.)
    /// for most test scenarios.
    pub terminal: Terminal<TestBackend>,
}
```

### Acceptance Criteria

1. `TestTerminal` struct has doc comment explaining purpose
2. `terminal` field has doc comment explaining why it's public
3. Usage examples show both wrapper methods and direct access
4. `cargo doc` generates proper documentation

### Testing

```bash
# Verify docs compile
cargo doc --no-deps

# Check clippy for doc warnings
cargo clippy -- -D warnings
```

### Notes

- This documents existing behavior, not a code change
- The public field is intentional for TEA View testing flexibility
- Consider adding `draw_with()` method in task 06 for cleaner API

---

## Completion Summary

**Status:** ❌ Not done

**Files Modified:**
- (pending)

**Testing Performed:**
- (pending)
