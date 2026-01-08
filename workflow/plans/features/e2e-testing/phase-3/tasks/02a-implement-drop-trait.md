## Task: Implement Drop Trait for FdemonSession

**Objective**: Add a `Drop` implementation to `FdemonSession` to ensure spawned fdemon processes are cleaned up even if tests panic or return early.

**Depends on**: 02-pty-test-utilities

### Scope

- `tests/e2e/pty_utils.rs`: Add `impl Drop for FdemonSession`

### Details

If a test panics before calling `quit()` or `kill()`, the spawned fdemon process continues running. Orphaned processes accumulate and can interfere with subsequent tests.

**Implementation:**
```rust
impl Drop for FdemonSession {
    fn drop(&mut self) {
        // Best-effort cleanup - ignore errors during drop
        let _ = self.kill();
    }
}
```

### Acceptance Criteria

1. `FdemonSession` implements `Drop` trait
2. `Drop::drop()` calls `kill()` with errors ignored
3. No orphaned fdemon processes after tests (verify with `pgrep fdemon`)
4. Tests that panic mid-session don't leave orphaned processes

### Testing

```rust
#[test]
#[should_panic]
#[ignore] // PTY test
fn test_panic_cleanup() {
    let fixture = TestFixture::simple_app();
    let _session = FdemonSession::spawn(&fixture.path()).unwrap();
    panic!("Test panic - session should still be cleaned up");
}
```

Manual verification:
```bash
# Run panic test and verify no orphaned process
cargo test --test e2e test_panic_cleanup -- --ignored
pgrep fdemon  # Should return empty
```

### Notes

- Drop implementations cannot return errors, so use `let _ = self.kill()` to ignore failures
- This is a critical safety feature for CI stability

### Review Source

- Risks & Tradeoffs Analyzer: "Missing Drop Implementation (HIGH)"
- ACTION_ITEMS.md Issue #1
