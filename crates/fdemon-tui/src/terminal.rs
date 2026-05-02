//! Terminal setup and restoration.
//!
//! Provides:
//! - [`install_panic_hook`] — restore the terminal on panic, including mouse
//!   capture if it was enabled.
//! - [`enable_mouse_capture`] / [`disable_mouse_capture`] — gated by an
//!   `AtomicBool` so disable is a no-op when enable was never called or
//!   failed (works around crossterm issue #613 on Windows).

use std::io::stdout;
use std::sync::atomic::{AtomicBool, Ordering};

use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use fdemon_core::prelude::*;
use tracing::warn;

/// Tracks whether [`enable_mouse_capture`] succeeded. Read by
/// [`disable_mouse_capture`] to skip the call entirely when capture was
/// never enabled — works around crossterm issue #613, which panics with
/// `TryFromIntError` on Windows when `DisableMouseCapture` is sent without
/// a prior `EnableMouseCapture`.
static MOUSE_CAPTURE_ON: AtomicBool = AtomicBool::new(false);

/// Install a panic hook that disables mouse capture (if enabled) and
/// restores the terminal before the panic propagates.
///
/// Wraps the existing panic hook so any pre-existing color-eyre / std hook
/// still runs after the terminal cleanup completes.
pub fn install_panic_hook() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        // Cleanup is best-effort — we are already panicking. Failures here
        // would be silently lost anyway because ratatui::restore() is also
        // best-effort.
        disable_mouse_capture();
        ratatui::restore();
        original_hook(panic_info);
    }));
}

/// Enable terminal mouse capture (button events, drag, scroll wheel).
///
/// Sends `?1000h ?1002h ?1003h ?1015h ?1006h` (the five sequences emitted
/// by `crossterm::event::EnableMouseCapture`). On success, sets the
/// `MOUSE_CAPTURE_ON` flag so the matching [`disable_mouse_capture`] call
/// later actually runs.
///
/// Returns an [`Error`] if the underlying `execute!` fails (terminal
/// doesn't support mouse, or stdout write failed). The caller should log
/// the failure and continue — the rest of the application works without
/// mouse support.
// Called by runner.rs once task-06 wires up the enable_mouse setting.
#[allow(dead_code)]
pub fn enable_mouse_capture() -> Result<()> {
    execute!(stdout(), EnableMouseCapture).map_err(|e| {
        warn!("failed to enable mouse capture: {e}");
        Error::terminal(format!("EnableMouseCapture failed: {e}"))
    })?;
    MOUSE_CAPTURE_ON.store(true, Ordering::SeqCst);
    Ok(())
}

/// Disable terminal mouse capture if it was previously enabled.
///
/// No-op if [`enable_mouse_capture`] was never called or returned an error.
/// This guards against crossterm issue #613, which panics on Windows when
/// `DisableMouseCapture` is sent without a prior `EnableMouseCapture`.
///
/// Errors from the underlying `execute!` are logged at `warn` level and
/// then swallowed — this function must never panic, including from inside
/// a panic hook.
pub fn disable_mouse_capture() {
    if !MOUSE_CAPTURE_ON.swap(false, Ordering::SeqCst) {
        return;
    }
    if let Err(e) = execute!(stdout(), DisableMouseCapture) {
        // Use eprintln when in a panic context? No — we must not write to
        // stdout in a panic; tracing is fine because it goes to the file
        // log via tracing-appender (stdout is owned by the TUI).
        warn!("failed to disable mouse capture: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reset the capture flag so each test starts clean. Tests in this file
    /// mutate global state and must run serially — the `serial_test` crate
    /// (already a dev-dependency) gates them.
    fn reset_flag() {
        MOUSE_CAPTURE_ON.store(false, Ordering::SeqCst);
    }

    #[test]
    #[serial_test::serial]
    fn test_disable_without_enable_is_noop() {
        reset_flag();
        // Must not panic, must not write any escape sequences. We can't
        // intercept stdout writes from execute! cleanly, so the
        // observable behavior is: no panic, and the flag is still false
        // afterwards.
        disable_mouse_capture();
        assert!(!MOUSE_CAPTURE_ON.load(Ordering::SeqCst));
    }

    #[test]
    #[serial_test::serial]
    fn test_disable_after_simulated_enable_clears_flag() {
        reset_flag();
        // Simulate a successful enable by setting the flag directly. We
        // cannot invoke the real enable_mouse_capture() in unit tests
        // because it writes to the test process's stdout (and on CI
        // that stdout is not a TTY).
        MOUSE_CAPTURE_ON.store(true, Ordering::SeqCst);
        disable_mouse_capture();
        // The flag must be cleared even if execute! fails (which it will
        // in non-TTY test environments).
        assert!(!MOUSE_CAPTURE_ON.load(Ordering::SeqCst));
    }

    #[test]
    #[serial_test::serial]
    fn test_repeated_disable_calls_are_safe() {
        reset_flag();
        disable_mouse_capture();
        disable_mouse_capture();
        disable_mouse_capture();
        assert!(!MOUSE_CAPTURE_ON.load(Ordering::SeqCst));
    }
}
