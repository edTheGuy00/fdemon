## Task: Improve TestTerminal API Encapsulation

**Objective**: Add a `draw_with()` method to TestTerminal for cleaner frame rendering, reducing need for direct terminal field access.

**Depends on**: Wave 2 complete

### Scope

- `src/tui/test_utils.rs`: Add `draw_with()` method
- `src/tui/render/tests.rs`: Update to use new method

### Details

Currently, full-frame rendering requires direct field access:

**Current pattern:**
```rust
term.terminal.draw(|frame| view(frame, &state))?;
```

**Add `draw_with()` method:**
```rust
impl TestTerminal {
    /// Draws a frame using a custom rendering function.
    ///
    /// This is useful for testing full-screen rendering (like `tui::view`)
    /// rather than individual widgets.
    ///
    /// # Arguments
    /// * `f` - A closure that receives a mutable Frame reference
    ///
    /// # Example
    /// ```ignore
    /// let mut term = TestTerminal::new();
    /// term.draw_with(|frame| view(frame, &state)).unwrap();
    /// assert!(term.buffer_contains("expected content"));
    /// ```
    pub fn draw_with<F>(&mut self, f: F) -> std::io::Result<()>
    where
        F: FnOnce(&mut Frame),
    {
        self.terminal.draw(f)?;
        Ok(())
    }
}
```

**Update render/tests.rs to use new API:**

```rust
// Before:
term.terminal.draw(|frame| view(frame, &state))?;

// After:
term.draw_with(|frame| view(frame, &state))?;
```

### Acceptance Criteria

1. `draw_with()` method added to TestTerminal
2. Method has comprehensive doc comment with example
3. `render/tests.rs` updated to use `draw_with()`
4. All render tests pass
5. Terminal field remains public (for edge cases, but documented)

### Testing

```bash
# Run test_utils tests
cargo test --lib test_utils

# Run render tests
cargo test --lib render

# Verify full test suite
cargo test --lib
```

### Notes

- Keep `terminal` field public - `draw_with()` is a convenience, not a replacement
- The `std::io::Result<()>` return type matches terminal.draw() signature
- This improves test ergonomics without breaking existing code

---

## Completion Summary

**Status:** ❌ Not done

**Files Modified:**
- (pending)

**Testing Performed:**
- (pending)
