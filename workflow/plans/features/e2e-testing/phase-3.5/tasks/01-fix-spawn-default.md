## Task: Fix FdemonSession::spawn() Default to TUI Mode

**Objective**: Change the default `spawn()` method to use TUI mode (no `--headless` flag) so PTY tests can verify TUI output.

**Depends on**: None (Critical Path)

### Scope

- `tests/e2e/pty_utils.rs`: Modify spawn methods

### Details

#### 1. Current Implementation (Broken)

```rust
// tests/e2e/pty_utils.rs:99-101
pub fn spawn(project_path: &Path) -> PtyResult<Self> {
    Self::spawn_with_args(project_path, &["--headless"])  // Outputs JSON, not TUI
}
```

#### 2. Fixed Implementation

```rust
/// Spawn fdemon in TUI mode for terminal interaction testing.
///
/// This spawns fdemon WITHOUT the `--headless` flag so the TUI renders
/// to the terminal, allowing tests to verify text output like headers,
/// status bars, and dialogs.
///
/// For tests that need JSON event output, use [`spawn_headless`](Self::spawn_headless).
pub fn spawn(project_path: &Path) -> PtyResult<Self> {
    Self::spawn_with_args(project_path, &[])  // TUI mode - renders to terminal
}

/// Spawn fdemon in headless mode for JSON event testing.
///
/// Headless mode outputs NDJSON events to stdout instead of rendering
/// the TUI. Use this for testing event emission, not TUI appearance.
///
/// For TUI interaction tests, use [`spawn`](Self::spawn).
pub fn spawn_headless(project_path: &Path) -> PtyResult<Self> {
    Self::spawn_with_args(project_path, &["--headless"])
}
```

#### 3. Update Documentation

Add clear guidance in the module docs:

```rust
//! ## Test Modes
//!
//! - **TUI Mode** (`spawn()`): Renders widgets to terminal. Use for verifying
//!   headers, status bars, dialogs, keyboard navigation.
//!
//! - **Headless Mode** (`spawn_headless()`): Outputs JSON events. Use for
//!   testing event emission and machine-readable output.
```

### Acceptance Criteria

1. `spawn()` no longer passes `--headless` flag
2. New `spawn_headless()` method available
3. Documentation clearly explains when to use each
4. Existing tests that need headless mode are updated to use `spawn_headless()`

### Testing

```bash
# Verify TUI output
cargo test --test e2e test_startup_shows_header -- --nocapture

# Should see actual terminal output, not JSON

# Run full suite
cargo test --test e2e
```

### Notes

- This is the critical fix - all other tasks depend on this
- Most tests should continue using `spawn()` (now TUI mode)
- Only tests specifically verifying JSON events need `spawn_headless()`

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `tests/e2e/pty_utils.rs` | Changed `spawn()` to use TUI mode (no `--headless`), added `spawn_headless()` method, updated module docs |

### Notable Decisions/Tradeoffs

1. **No tests required updating to use `spawn_headless()`**: All existing tests were using `spawn()` for TUI testing, so they benefit from the change to TUI mode. The only test using `--headless` explicitly was already using `spawn_with_args()` directly (line 953).

2. **Test failures are expected**: Some e2e tests now fail because they were incorrectly using headless mode before. These tests were trying to verify TUI output but couldn't because headless mode outputs JSON. This is the bug that this task fixes. The failing tests will need to be fixed in subsequent tasks (likely with timing adjustments or proper TUI interaction patterns).

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed (0.24s)
- `cargo test --test e2e pty_utils::tests` - Passed (9 tests passed, 6 ignored as expected)
- `cargo clippy -- -D warnings` - Passed (0.27s)
- `cargo test --test e2e` - Ran with 24 expected failures (tests that now correctly run in TUI mode but need timing/interaction fixes)

### Risks/Limitations

1. **Expected test failures**: 24 e2e tests now fail because they were designed for headless mode but are actually testing TUI features. These tests need to be updated in follow-up tasks to work with proper TUI mode. This is the intended outcome of this task - exposing the tests that were incorrectly using headless mode.

2. **Breaking change for external users**: If anyone was depending on `spawn()` defaulting to headless mode, they'll need to switch to `spawn_headless()`. However, this is in the test utilities, not production code, so the impact is limited to test code.
