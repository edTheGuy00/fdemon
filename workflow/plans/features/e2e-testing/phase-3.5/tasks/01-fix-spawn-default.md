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

**Status:** Not Started

**Files Modified:**
- (none yet)

**Testing Performed:**
- `cargo fmt` - Pending
- `cargo check` - Pending
- `cargo test --test e2e` - Pending

**Notable Decisions:**
- (none yet)

**Risks/Limitations:**
- (none yet)
