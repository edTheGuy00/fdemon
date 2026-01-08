## Task: Categorize TUI vs Headless Tests

**Objective**: Clearly categorize tests by mode (TUI vs headless) and update documentation to prevent future confusion.

**Depends on**: 01-fix-spawn-default

### Scope

- `tests/e2e/tui_interaction.rs`: Update test documentation
- `tests/e2e/tui_workflows.rs`: Update test documentation
- `tests/e2e/pty_utils.rs`: Module documentation

### Details

#### 1. Add Test Categories Documentation

Update `tests/e2e/tui_interaction.rs` header:

```rust
//! ## Test Categories
//!
//! ### TUI Tests (use `spawn()`)
//! Tests that verify terminal rendering and visual output:
//! - Startup header display
//! - Status bar content
//! - Device selector appearance
//! - Dialog rendering
//!
//! ### Event Tests (use `spawn_headless()`)
//! Tests that verify JSON event emission:
//! - Daemon connected events
//! - Session lifecycle events
//! - Error reporting format
//!
//! Most tests in this file are TUI tests using the default `spawn()`.
```

#### 2. Mark Tests with Category Comments

```rust
// === TUI TESTS ===
// These tests verify terminal rendering using spawn() (TUI mode)

#[tokio::test]
#[serial]
async fn test_startup_shows_header() {  // TUI Test
    let mut session = FdemonSession::spawn(&fixture.path())?;
    // ...
}

// === EVENT TESTS ===
// These tests verify JSON events using spawn_headless()

#[tokio::test]
#[serial]
async fn test_daemon_emits_connected_event() {  // Event Test
    let mut session = FdemonSession::spawn_headless(&fixture.path())?;
    // ...
}
```

#### 3. Update Workflow Tests Documentation

Update `tests/e2e/tui_workflows.rs`:

```rust
//! # TUI Workflow Tests
//!
//! Complex multi-step tests that exercise user journeys through the TUI.
//! All tests in this file use TUI mode (`spawn()`) to verify terminal output.
//!
//! ## When to Use Headless Mode
//!
//! Use `spawn_headless()` when:
//! - Testing JSON event format/content
//! - Testing machine-readable output
//! - NOT testing visual appearance
//!
//! ## Current Test Status
//!
//! Many workflow tests are marked `#[ignore]` because they require:
//! - Real Flutter devices (not available in CI)
//! - Full Flutter daemon (not mocked)
//!
//! These tests serve as documentation and can be run manually.
```

### Acceptance Criteria

1. Each test file has category documentation
2. Tests grouped by category with comments
3. Clear guidance on when to use each spawn method
4. `#[ignore]` tests have reason comments

### Testing

```bash
# Verify documentation compiles
cargo doc --test --no-deps

# Run TUI tests only
cargo test --test e2e "test_startup|test_device|test_quit"

# Run workflow tests
cargo test --test e2e workflow
```

---

## Completion Summary

**Status:** Not Started
