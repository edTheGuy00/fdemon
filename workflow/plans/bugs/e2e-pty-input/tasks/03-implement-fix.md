## Task: Implement Fix

**Objective**: Apply the working solution identified in Task 02.

**Depends on**: 02-test-alternative-sequences

### Scope

- Files depend on which solution works (identified in Task 02)

### Details

Implement the fix identified through experimentation, ensuring it doesn't break real terminal usage.

### Implementation Steps

1. **Apply the fix** from the working experiment

2. **Verify E2E tests pass**:
   ```bash
   cargo test test_toggle_ --test e2e
   ```

3. **Verify other E2E tests still pass**:
   ```bash
   cargo test --test e2e
   ```

4. **Verify real terminal still works**:
   - Run `cargo run` in a Flutter project
   - Test all key inputs manually
   - Confirm no double-processing or missed events

5. **Run full test suite**:
   ```bash
   cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings
   ```

### Acceptance Criteria

1. Fix implemented cleanly
2. E2E toggle tests pass (when #[ignore] removed)
3. Other E2E tests pass (no regressions)
4. Real terminal usage unaffected
5. All quality gates pass

### Possible Fixes (Based on Likely Solutions)

#### If Experiment A Works (Newline)

```rust
// pty_utils.rs
SpecialKey::Enter => b"\n",  // Changed from b"\r"
```

#### If Experiment C Works (send_line)

```rust
// pty_utils.rs
impl FdemonSession {
    /// Send Enter key using line-oriented method
    pub fn send_enter(&mut self) -> PtyResult<()> {
        self.session.send_line("")?;
        Ok(())
    }
}
```

Update tests to use `send_enter()` instead of `send_special(SpecialKey::Enter)`.

#### If Experiment E Works (Remove Filter)

```rust
// event.rs - Only if absolutely necessary
// Add comment explaining why filter is relaxed
Event::Key(key) => {
    // Accept all key event kinds for PTY compatibility
    // Some PTY implementations don't send KeyEventKind::Press
    Ok(Some(Message::Key(key)))
}
```

Add test to verify no double-processing in real terminal.

### Notes

- Prefer minimal changes
- Document why the change is needed
- Consider adding a comment in the code explaining the PTY behavior

---

## Completion Summary

**Status:** Not Started

**Files Modified:**
- (none yet)

**Solution Implemented:**

(To be filled after implementation)

**Testing Performed:**

(To be filled after implementation)
