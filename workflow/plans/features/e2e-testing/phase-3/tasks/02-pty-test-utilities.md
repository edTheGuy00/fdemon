## Task: Create PTY Test Utilities

**Objective**: Create a reusable utilities module for PTY-based TUI testing that wraps `expectrl` with fdemon-specific helpers.

**Depends on**: 01-add-pty-dependencies

### Scope

- `tests/e2e/pty_utils.rs`: **NEW** - PTY utilities module
- `tests/e2e/mod.rs`: Export the new module

### Details

Create `tests/e2e/pty_utils.rs` with the following utilities:

```rust
//! PTY-based TUI testing utilities
//!
//! Provides helpers for spawning fdemon in a pseudo-terminal
//! and interacting with it programmatically.

use expectrl::{Captures, Session, WaitStatus};
use std::path::Path;
use std::time::Duration;

/// Default timeout for expect operations
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// Result type for PTY operations
pub type PtyResult<T> = Result<T, Box<dyn std::error::Error>>;

/// Wrapper around expectrl::Session with fdemon-specific helpers
pub struct FdemonSession {
    session: Session,
    project_path: String,
}

impl FdemonSession {
    /// Spawn fdemon in headless mode for a given project
    pub fn spawn(project_path: &Path) -> PtyResult<Self> {
        // Build command: fdemon --headless <project_path>
        todo!()
    }

    /// Spawn fdemon with custom arguments
    pub fn spawn_with_args(project_path: &Path, args: &[&str]) -> PtyResult<Self> {
        todo!()
    }

    /// Wait for fdemon to show the header with project name
    pub fn expect_header(&mut self) -> PtyResult<()> {
        todo!()
    }

    /// Wait for device selector to appear
    pub fn expect_device_selector(&mut self) -> PtyResult<()> {
        todo!()
    }

    /// Wait for "Running" phase indicator
    pub fn expect_running(&mut self) -> PtyResult<()> {
        todo!()
    }

    /// Wait for "Reloading" phase indicator
    pub fn expect_reloading(&mut self) -> PtyResult<()> {
        todo!()
    }

    /// Wait for any output matching a pattern
    pub fn expect(&mut self, pattern: &str) -> PtyResult<Captures> {
        todo!()
    }

    /// Wait for output with custom timeout
    pub fn expect_timeout(&mut self, pattern: &str, timeout: Duration) -> PtyResult<Captures> {
        todo!()
    }

    /// Send a key press (single character)
    pub fn send_key(&mut self, key: char) -> PtyResult<()> {
        todo!()
    }

    /// Send special key (arrow, enter, escape, etc.)
    pub fn send_special(&mut self, key: SpecialKey) -> PtyResult<()> {
        todo!()
    }

    /// Send raw bytes (for complex key sequences)
    pub fn send_raw(&mut self, bytes: &[u8]) -> PtyResult<()> {
        todo!()
    }

    /// Get current terminal content (for snapshot testing)
    pub fn capture_screen(&mut self) -> PtyResult<String> {
        todo!()
    }

    /// Send quit command and wait for exit
    pub fn quit(&mut self) -> PtyResult<WaitStatus> {
        todo!()
    }

    /// Force kill the process
    pub fn kill(&mut self) -> PtyResult<()> {
        todo!()
    }
}

/// Special keys that can be sent to the terminal
pub enum SpecialKey {
    Enter,
    Escape,
    Tab,
    Backspace,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    PageUp,
    PageDown,
    Home,
    End,
    F(u8), // F1-F12
}

impl SpecialKey {
    /// Get the ANSI escape sequence for this key
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            SpecialKey::Enter => b"\r",
            SpecialKey::Escape => b"\x1b",
            SpecialKey::Tab => b"\t",
            SpecialKey::Backspace => b"\x7f",
            SpecialKey::ArrowUp => b"\x1b[A",
            SpecialKey::ArrowDown => b"\x1b[B",
            SpecialKey::ArrowRight => b"\x1b[C",
            SpecialKey::ArrowLeft => b"\x1b[D",
            SpecialKey::PageUp => b"\x1b[5~",
            SpecialKey::PageDown => b"\x1b[6~",
            SpecialKey::Home => b"\x1b[H",
            SpecialKey::End => b"\x1b[F",
            SpecialKey::F(n) => match n {
                1 => b"\x1bOP",
                2 => b"\x1bOQ",
                3 => b"\x1bOR",
                4 => b"\x1bOS",
                // ... etc
                _ => b"",
            },
        }
    }
}

/// Builder for test fixtures
pub struct TestFixture {
    fixture_name: &'static str,
}

impl TestFixture {
    /// Get the simple_app fixture
    pub fn simple_app() -> Self {
        Self { fixture_name: "simple_app" }
    }

    /// Get the error_app fixture
    pub fn error_app() -> Self {
        Self { fixture_name: "error_app" }
    }

    /// Get the path to this fixture
    pub fn path(&self) -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(self.fixture_name)
    }
}
```

### Acceptance Criteria

1. `FdemonSession::spawn()` successfully starts fdemon in a PTY
2. `expect_*` methods correctly identify UI states
3. `send_key()` and `send_special()` reliably send input
4. `capture_screen()` returns readable terminal content
5. Tests can be written using these utilities without low-level PTY details

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spawn_fdemon() {
        let fixture = TestFixture::simple_app();
        let mut session = FdemonSession::spawn(&fixture.path()).unwrap();

        // Should show header
        session.expect_header().unwrap();

        // Clean exit
        session.quit().unwrap();
    }

    #[test]
    fn test_special_key_bytes() {
        assert_eq!(SpecialKey::Enter.as_bytes(), b"\r");
        assert_eq!(SpecialKey::ArrowUp.as_bytes(), b"\x1b[A");
    }
}
```

### Notes

- PTY behavior differs between platforms; test on Linux (CI environment) primarily
- Consider using `#[ignore]` for slow PTY tests and running them separately
- The `capture_screen()` method may need ANSI escape code stripping
- Timeout values may need tuning based on CI performance

---

## Completion Summary

**Status:** Not Started

**Files Modified:**
- (none yet)

**Implementation Details:**

(to be filled after implementation)

**Testing Performed:**
- `cargo fmt` - Pending
- `cargo clippy` - Pending
- `cargo test` - Pending

**Notable Decisions:**
- (none yet)

**Risks/Limitations:**
- (none yet)
