//! PTY-based TUI testing utilities
//!
//! Provides helpers for spawning fdemon in a pseudo-terminal
//! and interacting with it programmatically.

#![allow(dead_code)] // Some utilities are for future tests

use expectrl::{Captures, Regex, Session};
use std::path::Path;
use std::time::Duration;

#[cfg(test)]
use serial_test::serial;

/// Default timeout for expect operations
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// Time to wait for graceful quit before force-killing
const QUIT_TIMEOUT_MS: u64 = 2000;
/// Interval between process state checks
const QUIT_POLL_INTERVAL_MS: u64 = 100;
/// Time to wait between kill attempts
const KILL_RETRY_DELAY_MS: u64 = 100;
/// Short delay for screen capture
const CAPTURE_DELAY_MS: u64 = 500;

/// Result type for PTY operations
pub type PtyResult<T> = Result<T, Box<dyn std::error::Error>>;

/// Wrapper around expectrl::Session with fdemon-specific helpers
pub struct FdemonSession {
    session: Session,
    project_path: String,
}

/// Find the fdemon binary, checking multiple locations.
///
/// Search order:
/// 1. `CARGO_BIN_EXE_fdemon` environment variable (set by `cargo test`)
/// 2. `target/release/fdemon` (release build)
/// 3. `target/debug/fdemon` (debug build)
///
/// # Errors
///
/// Returns an error with a helpful message if the binary is not found.
fn find_fdemon_binary() -> PtyResult<String> {
    // 1. Check CARGO_BIN_EXE_fdemon (set by cargo test)
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_fdemon") {
        if Path::new(&path).exists() {
            return Ok(path);
        }
    }

    let manifest_dir = env!("CARGO_MANIFEST_DIR");

    // 2. Check release build
    let release = format!("{}/target/release/fdemon", manifest_dir);
    if Path::new(&release).exists() {
        return Ok(release);
    }

    // 3. Check debug build
    let debug = format!("{}/target/debug/fdemon", manifest_dir);
    if Path::new(&debug).exists() {
        return Ok(debug);
    }

    Err("fdemon binary not found. Run `cargo build` or `cargo build --release` first.".into())
}

impl Drop for FdemonSession {
    fn drop(&mut self) {
        // Best-effort cleanup - ignore errors during drop
        let _ = self.kill();
    }
}

impl FdemonSession {
    /// Spawn fdemon in headless mode for the given Flutter project.
    ///
    /// This is a convenience wrapper around [`spawn_with_args`](Self::spawn_with_args)
    /// that automatically passes the `--headless` flag for non-interactive testing.
    ///
    /// # Arguments
    ///
    /// * `project_path` - Path to a Flutter project directory
    ///
    /// # Returns
    ///
    /// A new `FdemonSession` ready for interaction.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - fdemon binary not found (run `cargo build` first)
    /// - Project path is invalid
    /// - PTY spawn fails
    pub fn spawn(project_path: &Path) -> PtyResult<Self> {
        Self::spawn_with_args(project_path, &["--headless"])
    }

    /// Spawn fdemon with custom command-line arguments.
    ///
    /// Provides full control over fdemon launch arguments for testing
    /// different modes and configurations.
    ///
    /// # Arguments
    ///
    /// * `project_path` - Path to a Flutter project directory
    /// * `args` - Command-line arguments to pass to fdemon (e.g., `["--headless"]`)
    ///
    /// # Returns
    ///
    /// A new `FdemonSession` ready for interaction.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - fdemon binary not found (run `cargo build` or `cargo build --release`)
    /// - Project path is invalid
    /// - PTY spawn fails
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use std::path::Path;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let project_path = Path::new(".");
    /// // Spawn in TUI mode (no --headless) for visual testing
    /// let mut session = FdemonSession::spawn_with_args(project_path, &[])?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn spawn_with_args(project_path: &Path, args: &[&str]) -> PtyResult<Self> {
        // Get the path to the fdemon binary with fallback logic
        let binary_path = find_fdemon_binary()?;

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

    /// Wait for fdemon to display its header with the project name.
    ///
    /// Waits for the header banner containing "Flutter Demon" or "fdemon"
    /// to appear in the terminal output. This typically happens shortly
    /// after startup.
    ///
    /// # Errors
    ///
    /// Returns error if header is not found within the default timeout.
    pub fn expect_header(&mut self) -> PtyResult<()> {
        // The header typically contains "Flutter Demon" or project name
        self.session.expect(Regex("Flutter Demon|fdemon"))?;
        Ok(())
    }

    /// Wait for the device selector UI to appear.
    ///
    /// Matches output containing "Select a device" or "Available device(s)"
    /// which indicates the device selection modal is displayed.
    ///
    /// # Errors
    ///
    /// Returns error if device selector is not shown within the default timeout.
    pub fn expect_device_selector(&mut self) -> PtyResult<()> {
        // Device selector shows "Select a device" or similar
        self.session
            .expect(Regex("Select.*device|Available.*device"))?;
        Ok(())
    }

    /// Wait for the "Running" phase indicator to appear.
    ///
    /// Waits for the status bar to show "Running" or "RUNNING",
    /// indicating that the Flutter app has started successfully.
    ///
    /// # Errors
    ///
    /// Returns error if "Running" status is not found within the default timeout.
    pub fn expect_running(&mut self) -> PtyResult<()> {
        self.session.expect(Regex("Running|RUNNING"))?;
        Ok(())
    }

    /// Wait for the "Reloading" phase indicator to appear.
    ///
    /// Waits for the status bar to show "Reloading" or "RELOADING",
    /// indicating that a hot reload operation is in progress.
    ///
    /// # Errors
    ///
    /// Returns error if "Reloading" status is not found within the default timeout.
    pub fn expect_reloading(&mut self) -> PtyResult<()> {
        self.session.expect(Regex("Reloading|RELOADING"))?;
        Ok(())
    }

    /// Wait for any output matching a regex pattern.
    ///
    /// Uses the default timeout (`DEFAULT_TIMEOUT`). For custom timeout,
    /// use [`expect_timeout`](Self::expect_timeout).
    ///
    /// # Arguments
    ///
    /// * `pattern` - Regular expression to match against terminal output
    ///
    /// # Returns
    ///
    /// Captured matches if pattern is found.
    ///
    /// # Errors
    ///
    /// Returns error if pattern is not matched within the timeout period.
    pub fn expect(&mut self, pattern: &str) -> PtyResult<Captures> {
        self.expect_timeout(pattern, DEFAULT_TIMEOUT)
    }

    /// Wait for output matching a pattern with a custom timeout.
    ///
    /// Temporarily overrides the default timeout for this operation only.
    ///
    /// # Arguments
    ///
    /// * `pattern` - Regular expression to match against terminal output
    /// * `timeout` - Maximum time to wait for the pattern
    ///
    /// # Returns
    ///
    /// Captured matches if pattern is found.
    ///
    /// # Errors
    ///
    /// Returns error if pattern is not matched within the specified timeout.
    pub fn expect_timeout(&mut self, pattern: &str, timeout: Duration) -> PtyResult<Captures> {
        // Set timeout for this operation
        self.session.set_expect_timeout(Some(timeout));

        // Try to match the pattern (as regex)
        let result = self.session.expect(Regex(pattern));

        // Restore default timeout
        self.session.set_expect_timeout(Some(DEFAULT_TIMEOUT));

        Ok(result?)
    }

    /// Send a single character key press to fdemon.
    ///
    /// # Arguments
    ///
    /// * `key` - Character to send (e.g., 'r' for reload, 'q' for quit)
    ///
    /// # Errors
    ///
    /// Returns error if the PTY session fails to send the input.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use std::path::Path;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let project_path = Path::new(".");
    /// let mut session = FdemonSession::spawn(project_path)?;
    /// session.send_key('r')?; // Trigger hot reload
    /// # Ok(())
    /// # }
    /// ```
    pub fn send_key(&mut self, key: char) -> PtyResult<()> {
        self.session.send(key.to_string())?;
        Ok(())
    }

    /// Send a special key press (arrow keys, Enter, Escape, etc.).
    ///
    /// Sends ANSI escape sequences for keys that don't have a single
    /// character representation.
    ///
    /// # Arguments
    ///
    /// * `key` - The special key to send (see [`SpecialKey`] enum)
    ///
    /// # Errors
    ///
    /// Returns error if the PTY session fails to send the input.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use std::path::Path;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let project_path = Path::new(".");
    /// let mut session = FdemonSession::spawn(project_path)?;
    /// session.send_special(SpecialKey::ArrowDown)?;
    /// session.send_special(SpecialKey::Enter)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn send_special(&mut self, key: SpecialKey) -> PtyResult<()> {
        self.send_raw(key.as_bytes())
    }

    /// Send raw bytes to the PTY (for complex key sequences).
    ///
    /// This is a low-level method for sending arbitrary byte sequences.
    /// Most users should prefer [`send_key`](Self::send_key) or
    /// [`send_special`](Self::send_special).
    ///
    /// # Arguments
    ///
    /// * `bytes` - Raw bytes to send to the PTY
    ///
    /// # Errors
    ///
    /// Returns error if the PTY session fails to send the input.
    pub fn send_raw(&mut self, bytes: &[u8]) -> PtyResult<()> {
        self.session.send(bytes)?;
        Ok(())
    }

    /// Get current terminal content (for snapshot testing)
    ///
    /// Attempts to capture available terminal output with a short timeout.
    /// This is a best-effort operation that may return empty if no output
    /// is available within the timeout period (500ms).
    ///
    /// # Returns
    ///
    /// - `Ok(String)` with captured content (may be empty)
    /// - `Err` only on PTY session errors (not timeout)
    ///
    /// # Note
    ///
    /// This method is designed for snapshot testing and debugging.
    /// For reliable content verification with specific patterns, use `expect()` instead,
    /// which waits for specific content and fails if not found.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use std::path::Path;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let fixture_path = Path::new(".");
    /// let mut session = FdemonSession::spawn(fixture_path)?;
    ///
    /// // Wait for startup
    /// std::thread::sleep(std::time::Duration::from_millis(500));
    ///
    /// // Capture whatever is on screen
    /// let screen = session.capture_screen()?;
    ///
    /// // Or use expect for specific content
    /// session.expect("Flutter Demon")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn capture_screen(&mut self) -> PtyResult<String> {
        // Save current timeout
        let original_timeout = Some(DEFAULT_TIMEOUT);

        // Use a shorter timeout for capture
        let capture_timeout = Duration::from_millis(CAPTURE_DELAY_MS);
        self.session.set_expect_timeout(Some(capture_timeout));

        // Try to match any non-empty content
        let result = match self.session.expect(Regex(".+")) {
            Ok(found) => {
                // Get the matched content (not the bytes before the match)
                let bytes = found.get(0).unwrap_or(&[]);
                Ok(String::from_utf8_lossy(bytes).to_string())
            }
            Err(_) => {
                // Timeout is expected if no output is available
                // This is not an error, just return empty string
                Ok(String::new())
            }
        };

        // Restore original timeout
        self.session.set_expect_timeout(original_timeout);

        result
    }

    /// Send quit command ('q' + 'y') and wait for graceful exit.
    ///
    /// Sends 'q' key to initiate quit, followed by 'y' to confirm
    /// the quit dialog (if shown). This handles both cases:
    /// - Sessions running: 'q' shows dialog, 'y' confirms
    /// - No sessions: 'q' quits directly, 'y' is ignored
    ///
    /// If the process doesn't exit within the timeout period, it will
    /// be forcefully killed with [`kill`](Self::kill).
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Unable to send quit command
    /// - Process doesn't terminate even after force kill
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use std::path::Path;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let project_path = Path::new(".");
    /// let mut session = FdemonSession::spawn(project_path)?;
    /// // ... interact with fdemon ...
    /// session.quit()?; // Clean shutdown
    /// # Ok(())
    /// # }
    /// ```
    pub fn quit(&mut self) -> PtyResult<()> {
        // Send 'q' to initiate quit (may show confirmation dialog)
        self.send_key('q')?;

        // Brief pause for dialog to appear
        std::thread::sleep(Duration::from_millis(QUIT_POLL_INTERVAL_MS));

        // Send 'y' to confirm quit (if dialog appeared, otherwise ignored)
        self.send_key('y')?;

        // Wait for graceful shutdown with polling
        let iterations = QUIT_TIMEOUT_MS / QUIT_POLL_INTERVAL_MS;
        for _ in 0..iterations {
            std::thread::sleep(Duration::from_millis(QUIT_POLL_INTERVAL_MS));
            if !self.session.is_alive()? {
                return Ok(());
            }
        }

        // Still alive after timeout, force kill
        self.kill()?;

        // Verify termination
        for _ in 0..10 {
            std::thread::sleep(Duration::from_millis(QUIT_POLL_INTERVAL_MS));
            if !self.session.is_alive()? {
                return Ok(());
            }
        }

        Err("Process did not terminate after kill".into())
    }

    /// Force kill the fdemon process immediately.
    ///
    /// Sends interrupt signals (Ctrl+C, Ctrl+D) to forcefully terminate
    /// the process. This is called automatically if [`quit`](Self::quit)
    /// times out.
    ///
    /// # Errors
    ///
    /// Returns error if unable to send interrupt signals.
    ///
    /// # Note
    ///
    /// Prefer [`quit`](Self::quit) for graceful shutdown. This method
    /// should only be used when immediate termination is required.
    pub fn kill(&mut self) -> PtyResult<()> {
        // Send Ctrl+C to interrupt the process
        self.send_raw(b"\x03")?;
        std::thread::sleep(Duration::from_millis(KILL_RETRY_DELAY_MS));

        // If still alive, send Ctrl+D (EOF)
        if self.session.is_alive()? {
            self.send_raw(b"\x04")?;
        }

        Ok(())
    }

    /// Get a mutable reference to the underlying PTY session.
    ///
    /// Provides direct access to the `expectrl::Session` for advanced
    /// operations not covered by the helper methods.
    ///
    /// # Returns
    ///
    /// Mutable reference to the underlying `Session`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use std::path::Path;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let project_path = Path::new(".");
    /// let mut session = FdemonSession::spawn(project_path)?;
    /// // Use expectrl API directly
    /// let raw_session = session.session_mut();
    /// // ... perform advanced operations ...
    /// # Ok(())
    /// # }
    /// ```
    pub fn session_mut(&mut self) -> &mut Session {
        &mut self.session
    }

    /// Get the project path this session is running.
    ///
    /// # Returns
    ///
    /// The path to the Flutter project passed to [`spawn`](Self::spawn).
    pub fn project_path(&self) -> &str {
        &self.project_path
    }
}

/// Special keys that can be sent to the terminal.
///
/// Represents non-character keys like arrows, function keys, etc.
/// Each variant maps to the appropriate ANSI escape sequence.
///
/// # Example
///
/// ```no_run
/// # use std::path::Path;
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # let project_path = Path::new(".");
/// let mut session = FdemonSession::spawn(project_path)?;
/// session.send_special(SpecialKey::ArrowDown)?;
/// session.send_special(SpecialKey::Enter)?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialKey {
    /// Enter/Return key
    Enter,
    /// Escape key
    Escape,
    /// Tab key
    Tab,
    /// Backspace key
    Backspace,
    /// Up arrow key
    ArrowUp,
    /// Down arrow key
    ArrowDown,
    /// Left arrow key
    ArrowLeft,
    /// Right arrow key
    ArrowRight,
    /// Page Up key
    PageUp,
    /// Page Down key
    PageDown,
    /// Home key
    Home,
    /// End key
    End,
    /// Function key (F1-F12)
    F(u8),
}

impl SpecialKey {
    /// Get the ANSI escape sequence for this key.
    ///
    /// # Returns
    ///
    /// Byte slice containing the ANSI escape sequence for this key.
    /// For invalid function keys (F > 12), returns an empty slice.
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

/// Builder for accessing test fixture projects.
///
/// Provides convenient access to Flutter test projects in `tests/fixtures/`.
/// Each fixture represents a different Flutter project configuration for testing.
///
/// # Example
///
/// ```no_run
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let fixture = TestFixture::simple_app();
/// let mut session = FdemonSession::spawn(&fixture.path())?;
/// # Ok(())
/// # }
/// ```
pub struct TestFixture {
    fixture_name: &'static str,
}

impl TestFixture {
    /// Get the `simple_app` test fixture.
    ///
    /// A basic Flutter application with minimal dependencies,
    /// suitable for testing core functionality.
    pub fn simple_app() -> Self {
        Self {
            fixture_name: "simple_app",
        }
    }

    /// Get the `error_app` test fixture.
    ///
    /// A Flutter application that contains intentional errors,
    /// useful for testing error handling and display.
    pub fn error_app() -> Self {
        Self {
            fixture_name: "error_app",
        }
    }

    /// Get the `multi_module` test fixture.
    ///
    /// A Flutter project with multiple modules,
    /// for testing package discovery and navigation.
    pub fn multi_module() -> Self {
        Self {
            fixture_name: "multi_module",
        }
    }

    /// Get the `plugin_with_example` test fixture.
    ///
    /// A Flutter plugin project with an example app,
    /// for testing plugin project detection and running.
    pub fn plugin_with_example() -> Self {
        Self {
            fixture_name: "plugin_with_example",
        }
    }

    /// Get the filesystem path to this fixture.
    ///
    /// # Returns
    ///
    /// Absolute path to the fixture directory in `tests/fixtures/`.
    pub fn path(&self) -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(self.fixture_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test-specific delays
    const TEST_STARTUP_DELAY_MS: u64 = 500;
    const TEST_KEY_PROCESSING_DELAY_MS: u64 = 200;

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
    fn test_special_key_equality() {
        // Test PartialEq implementation
        assert_eq!(SpecialKey::Enter, SpecialKey::Enter);
        assert_eq!(SpecialKey::Escape, SpecialKey::Escape);
        assert_eq!(SpecialKey::F(5), SpecialKey::F(5));

        // Test inequality
        assert_ne!(SpecialKey::Enter, SpecialKey::Escape);
        assert_ne!(SpecialKey::ArrowUp, SpecialKey::ArrowDown);
        assert_ne!(SpecialKey::F(1), SpecialKey::F(2));
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
    #[serial]
    fn test_spawn_fdemon() {
        let fixture = TestFixture::simple_app();
        let mut session = FdemonSession::spawn(&fixture.path()).unwrap();

        // Should be able to capture some output
        std::thread::sleep(Duration::from_millis(TEST_STARTUP_DELAY_MS));

        // Clean exit
        session.kill().unwrap();
    }

    #[test]
    #[ignore]
    #[serial]
    fn test_spawn_with_custom_args() {
        let fixture = TestFixture::simple_app();
        let mut session = FdemonSession::spawn_with_args(&fixture.path(), &["--headless"]).unwrap();

        // Should be able to capture some output
        std::thread::sleep(Duration::from_millis(TEST_STARTUP_DELAY_MS));

        // Clean exit
        session.kill().unwrap();
    }

    #[test]
    #[ignore]
    #[serial]
    fn test_send_key() {
        let fixture = TestFixture::simple_app();
        let mut session = FdemonSession::spawn(&fixture.path()).unwrap();

        // Wait a bit for startup
        std::thread::sleep(Duration::from_millis(TEST_STARTUP_DELAY_MS));

        // Send some keys
        session.send_key('r').unwrap();
        session.send_key('q').unwrap();

        // Give it time to process
        std::thread::sleep(Duration::from_millis(TEST_KEY_PROCESSING_DELAY_MS));

        // Clean exit
        session.kill().unwrap();
    }

    #[test]
    #[ignore]
    #[serial]
    fn test_send_special_keys() {
        let fixture = TestFixture::simple_app();
        let mut session = FdemonSession::spawn(&fixture.path()).unwrap();

        // Wait a bit for startup
        std::thread::sleep(Duration::from_millis(TEST_STARTUP_DELAY_MS));

        // Send special keys
        session.send_special(SpecialKey::ArrowDown).unwrap();
        session.send_special(SpecialKey::Enter).unwrap();
        session.send_special(SpecialKey::Escape).unwrap();

        // Give it time to process
        std::thread::sleep(Duration::from_millis(TEST_KEY_PROCESSING_DELAY_MS));

        // Clean exit
        session.kill().unwrap();
    }

    #[test]
    #[ignore]
    #[serial]
    fn test_capture_screen() {
        let fixture = TestFixture::simple_app();
        // Spawn in TUI mode (no --headless) so we get actual screen content
        let mut session = FdemonSession::spawn_with_args(&fixture.path(), &[]).unwrap();

        // First, wait for the header to ensure fdemon has started
        // This ensures there's actually content to capture
        session.expect_header().unwrap();

        // Now capture screen - should have the header content
        let content = session.capture_screen().unwrap();

        // Should have some content after header is shown
        assert!(
            !content.is_empty(),
            "Captured screen should contain header content"
        );

        // Verify it contains expected TUI content (project name with ANSI codes)
        // The content will be full of ANSI escape codes, so just verify basic structure
        assert!(
            content.contains("simple_app"),
            "Screen should contain project name, got: {}",
            &content[..content.len().min(200)] // Show first 200 chars for debugging
        );

        // Clean exit
        session.kill().unwrap();
    }

    #[test]
    #[ignore]
    #[serial]
    fn test_quit() {
        let fixture = TestFixture::simple_app();
        let mut session = FdemonSession::spawn(&fixture.path()).unwrap();

        // Wait a bit for startup
        std::thread::sleep(Duration::from_millis(TEST_STARTUP_DELAY_MS));

        // Quit and wait for exit
        session.quit().unwrap();
    }
}
