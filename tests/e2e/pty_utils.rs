//! PTY-based TUI testing utilities
//!
//! Provides helpers for spawning fdemon in a pseudo-terminal
//! and interacting with it programmatically.

use expectrl::{Captures, Regex, Session};
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
        Self::spawn_with_args(project_path, &["--headless"])
    }

    /// Spawn fdemon with custom arguments
    pub fn spawn_with_args(project_path: &Path, args: &[&str]) -> PtyResult<Self> {
        // Get the path to the fdemon binary
        let binary_path = std::env::var("CARGO_BIN_EXE_fdemon").unwrap_or_else(|_| {
            // Fallback to cargo build artifact
            let manifest_dir = env!("CARGO_MANIFEST_DIR");
            format!("{}/target/debug/fdemon", manifest_dir)
        });

        // Build command arguments
        let mut full_args = Vec::new();
        full_args.extend_from_slice(args);
        full_args.push(project_path.to_str().ok_or("Invalid project path")?);

        // Spawn the process in a PTY directly
        let mut cmd = std::process::Command::new(&binary_path);
        cmd.args(&full_args);

        let session = Session::spawn(cmd)?;

        Ok(FdemonSession {
            session,
            project_path: project_path.to_string_lossy().to_string(),
        })
    }

    /// Wait for fdemon to show the header with project name
    pub fn expect_header(&mut self) -> PtyResult<()> {
        // The header typically contains "Flutter Demon" or project name
        self.session.expect(Regex("Flutter Demon|fdemon"))?;
        Ok(())
    }

    /// Wait for device selector to appear
    pub fn expect_device_selector(&mut self) -> PtyResult<()> {
        // Device selector shows "Select a device" or similar
        self.session
            .expect(Regex("Select.*device|Available.*device"))?;
        Ok(())
    }

    /// Wait for "Running" phase indicator
    pub fn expect_running(&mut self) -> PtyResult<()> {
        self.session.expect(Regex("Running|RUNNING"))?;
        Ok(())
    }

    /// Wait for "Reloading" phase indicator
    pub fn expect_reloading(&mut self) -> PtyResult<()> {
        self.session.expect(Regex("Reloading|RELOADING"))?;
        Ok(())
    }

    /// Wait for any output matching a pattern
    pub fn expect(&mut self, pattern: &str) -> PtyResult<Captures> {
        self.expect_timeout(pattern, DEFAULT_TIMEOUT)
    }

    /// Wait for output with custom timeout
    pub fn expect_timeout(&mut self, pattern: &str, timeout: Duration) -> PtyResult<Captures> {
        // Set timeout for this operation
        self.session.set_expect_timeout(Some(timeout));

        // Try to match the pattern (as regex)
        let result = self.session.expect(Regex(pattern));

        // Restore default timeout
        self.session.set_expect_timeout(Some(DEFAULT_TIMEOUT));

        Ok(result?)
    }

    /// Send a key press (single character)
    pub fn send_key(&mut self, key: char) -> PtyResult<()> {
        self.session.send(&key.to_string())?;
        Ok(())
    }

    /// Send special key (arrow, enter, escape, etc.)
    pub fn send_special(&mut self, key: SpecialKey) -> PtyResult<()> {
        self.send_raw(key.as_bytes())
    }

    /// Send raw bytes (for complex key sequences)
    pub fn send_raw(&mut self, bytes: &[u8]) -> PtyResult<()> {
        self.session.send(bytes)?;
        Ok(())
    }

    /// Get current terminal content (for snapshot testing)
    pub fn capture_screen(&mut self) -> PtyResult<String> {
        // Try to read available output with a short timeout
        // If nothing is available, return empty string
        match self.session.expect(Regex(".*")) {
            Ok(found) => {
                let bytes = found.before();
                Ok(String::from_utf8_lossy(bytes).to_string())
            }
            Err(_) => Ok(String::new()),
        }
    }

    /// Send quit command and wait for exit
    pub fn quit(&mut self) -> PtyResult<()> {
        // Send 'q' to quit
        self.send_key('q')?;

        // Give it a moment to process
        std::thread::sleep(Duration::from_millis(500));

        // Check if process is still alive
        let alive = self.session.is_alive()?;
        if alive {
            // Still alive, try to force kill
            self.kill()?;
            std::thread::sleep(Duration::from_millis(100));
        }

        Ok(())
    }

    /// Force kill the process
    pub fn kill(&mut self) -> PtyResult<()> {
        // Send Ctrl+C to interrupt the process
        self.send_raw(b"\x03")?;
        std::thread::sleep(Duration::from_millis(100));

        // If still alive, send Ctrl+D (EOF)
        if self.session.is_alive()? {
            self.send_raw(b"\x04")?;
        }

        Ok(())
    }

    /// Get a reference to the underlying session for advanced operations
    pub fn session_mut(&mut self) -> &mut Session {
        &mut self.session
    }

    /// Get the project path this session is running
    pub fn project_path(&self) -> &str {
        &self.project_path
    }
}

/// Special keys that can be sent to the terminal
#[derive(Debug, Clone, Copy)]
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
                5 => b"\x1b[15~",
                6 => b"\x1b[17~",
                7 => b"\x1b[18~",
                8 => b"\x1b[19~",
                9 => b"\x1b[20~",
                10 => b"\x1b[21~",
                11 => b"\x1b[23~",
                12 => b"\x1b[24~",
                _ => b"", // Invalid function key
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
        Self {
            fixture_name: "simple_app",
        }
    }

    /// Get the error_app fixture
    pub fn error_app() -> Self {
        Self {
            fixture_name: "error_app",
        }
    }

    /// Get the multi_module fixture
    pub fn multi_module() -> Self {
        Self {
            fixture_name: "multi_module",
        }
    }

    /// Get the plugin_with_example fixture
    pub fn plugin_with_example() -> Self {
        Self {
            fixture_name: "plugin_with_example",
        }
    }

    /// Get the path to this fixture
    pub fn path(&self) -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(self.fixture_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_special_key_bytes() {
        assert_eq!(SpecialKey::Enter.as_bytes(), b"\r");
        assert_eq!(SpecialKey::Escape.as_bytes(), b"\x1b");
        assert_eq!(SpecialKey::Tab.as_bytes(), b"\t");
        assert_eq!(SpecialKey::Backspace.as_bytes(), b"\x7f");
        assert_eq!(SpecialKey::ArrowUp.as_bytes(), b"\x1b[A");
        assert_eq!(SpecialKey::ArrowDown.as_bytes(), b"\x1b[B");
        assert_eq!(SpecialKey::ArrowRight.as_bytes(), b"\x1b[C");
        assert_eq!(SpecialKey::ArrowLeft.as_bytes(), b"\x1b[D");
        assert_eq!(SpecialKey::PageUp.as_bytes(), b"\x1b[5~");
        assert_eq!(SpecialKey::PageDown.as_bytes(), b"\x1b[6~");
        assert_eq!(SpecialKey::Home.as_bytes(), b"\x1b[H");
        assert_eq!(SpecialKey::End.as_bytes(), b"\x1b[F");
    }

    #[test]
    fn test_special_key_function_keys() {
        assert_eq!(SpecialKey::F(1).as_bytes(), b"\x1bOP");
        assert_eq!(SpecialKey::F(2).as_bytes(), b"\x1bOQ");
        assert_eq!(SpecialKey::F(3).as_bytes(), b"\x1bOR");
        assert_eq!(SpecialKey::F(4).as_bytes(), b"\x1bOS");
        assert_eq!(SpecialKey::F(5).as_bytes(), b"\x1b[15~");
        assert_eq!(SpecialKey::F(12).as_bytes(), b"\x1b[24~");
        assert_eq!(SpecialKey::F(99).as_bytes(), b""); // Invalid
    }

    #[test]
    fn test_fixture_paths() {
        let simple = TestFixture::simple_app();
        let error = TestFixture::error_app();
        let multi = TestFixture::multi_module();
        let plugin = TestFixture::plugin_with_example();

        assert!(simple.path().ends_with("tests/fixtures/simple_app"));
        assert!(error.path().ends_with("tests/fixtures/error_app"));
        assert!(multi.path().ends_with("tests/fixtures/multi_module"));
        assert!(plugin
            .path()
            .ends_with("tests/fixtures/plugin_with_example"));
    }

    #[test]
    fn test_fixture_exists() {
        let simple = TestFixture::simple_app();
        let error = TestFixture::error_app();

        // Verify fixtures exist
        assert!(
            simple.path().exists(),
            "simple_app fixture should exist at {:?}",
            simple.path()
        );
        assert!(
            error.path().exists(),
            "error_app fixture should exist at {:?}",
            error.path()
        );
    }

    // PTY tests are marked as #[ignore] because they require the binary to be built
    // and may be slow. Run with: cargo test --test e2e -- --ignored

    #[test]
    #[ignore]
    fn test_spawn_fdemon() {
        let fixture = TestFixture::simple_app();
        let mut session = FdemonSession::spawn(&fixture.path()).unwrap();

        // Should be able to capture some output
        std::thread::sleep(Duration::from_millis(500));

        // Clean exit
        session.kill().unwrap();
    }

    #[test]
    #[ignore]
    fn test_spawn_with_custom_args() {
        let fixture = TestFixture::simple_app();
        let mut session = FdemonSession::spawn_with_args(&fixture.path(), &["--headless"]).unwrap();

        // Should be able to capture some output
        std::thread::sleep(Duration::from_millis(500));

        // Clean exit
        session.kill().unwrap();
    }

    #[test]
    #[ignore]
    fn test_send_key() {
        let fixture = TestFixture::simple_app();
        let mut session = FdemonSession::spawn(&fixture.path()).unwrap();

        // Wait a bit for startup
        std::thread::sleep(Duration::from_millis(500));

        // Send some keys
        session.send_key('r').unwrap();
        session.send_key('q').unwrap();

        // Give it time to process
        std::thread::sleep(Duration::from_millis(200));

        // Clean exit
        session.kill().unwrap();
    }

    #[test]
    #[ignore]
    fn test_send_special_keys() {
        let fixture = TestFixture::simple_app();
        let mut session = FdemonSession::spawn(&fixture.path()).unwrap();

        // Wait a bit for startup
        std::thread::sleep(Duration::from_millis(500));

        // Send special keys
        session.send_special(SpecialKey::ArrowDown).unwrap();
        session.send_special(SpecialKey::Enter).unwrap();
        session.send_special(SpecialKey::Escape).unwrap();

        // Give it time to process
        std::thread::sleep(Duration::from_millis(200));

        // Clean exit
        session.kill().unwrap();
    }

    #[test]
    #[ignore]
    fn test_capture_screen() {
        let fixture = TestFixture::simple_app();
        let mut session = FdemonSession::spawn(&fixture.path()).unwrap();

        // Wait for some output
        std::thread::sleep(Duration::from_millis(500));

        // Capture screen
        let content = session.capture_screen().unwrap();

        // Should have some content
        assert!(!content.is_empty(), "Captured screen should not be empty");

        // Clean exit
        session.kill().unwrap();
    }

    #[test]
    #[ignore]
    fn test_quit() {
        let fixture = TestFixture::simple_app();
        let mut session = FdemonSession::spawn(&fixture.path()).unwrap();

        // Wait a bit for startup
        std::thread::sleep(Duration::from_millis(500));

        // Quit and wait for exit
        session.quit().unwrap();
    }
}
