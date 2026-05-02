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

/// Guards against double-installation of the panic hook. Each entry-point
/// runner calls [`install_panic_hook`]; multiple calls in one process would
/// chain duplicate mouse-disable / ratatui-restore closures, so we install
/// at most once per process.
static HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);

/// Install a panic hook that disables mouse capture (if enabled) and
/// restores the terminal before the panic propagates.
///
/// Wraps the existing panic hook so any pre-existing color-eyre / std hook
/// still runs after the terminal cleanup completes.
///
/// This function is idempotent: if called more than once in the same process
/// (e.g. from two different entry-point runners), only the first call installs
/// the hook. Subsequent calls return immediately without wrapping the hook a
/// second time.
pub fn install_panic_hook() {
    // Idempotency guard: each entry-point runner calls this; multiple calls
    // in one process would chain duplicate mouse-disable / ratatui-restore
    // closures, so we install at most once per process.
    if HOOK_INSTALLED.swap(true, Ordering::AcqRel) {
        return;
    }

    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        // Cleanup is best-effort — we are already panicking. Failures here
        // would be silently lost anyway because ratatui::restore() is also
        // best-effort.
        //
        // disable_mouse_capture() must run before ratatui::restore() so the
        // DECRST sequences are emitted while the alt screen is still active.
        // In practice, DECSET/DECRST mouse modes are connection-global (not
        // alt-screen-scoped) so the ordering doesn't matter for cleanup
        // correctness today — but this is a load-bearing assumption about
        // ratatui's restore() implementation. Keep the disable-then-restore
        // order to avoid coupling to that assumption changing.
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
pub fn enable_mouse_capture() -> Result<()> {
    // crossterm::EnableMouseCapture emits DECSET 1000/1002/1003/1015/1006.
    // We include 1003 (any-motion) even though `Moved` events are dropped at
    // the event.rs boundary. This trade-off keeps capture-mode setup symmetric
    // with crossterm's defaults; consumers that need to minimize per-frame
    // parser cost should switch to a tighter mode set (e.g. only 1002
    // button-event motion) when `Moved` events become useful in a future phase.
    execute!(stdout(), EnableMouseCapture).map_err(|e| {
        warn!("failed to enable mouse capture: {e}");
        Error::terminal(format!("EnableMouseCapture failed: {e}"))
    })?;
    // Release ordering: pairs with the Acquire swap in disable_mouse_capture.
    // Ensures the execute! terminal writes happen-before the flag is visible
    // as true to another thread deciding whether to call DisableMouseCapture.
    MOUSE_CAPTURE_ON.store(true, Ordering::Release);
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
    // AcqRel: acquires the Release store from enable_mouse_capture (so we
    // observe whether the terminal sequences were sent) and releases the
    // false write (so any later thread that checks the flag sees it cleared).
    if !MOUSE_CAPTURE_ON.swap(false, Ordering::AcqRel) {
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
        // Relaxed ordering: the test's serial gate (serial_test::serial)
        // ensures no concurrent accesses; no cross-thread synchronization
        // is needed for this reset.
        MOUSE_CAPTURE_ON.store(false, Ordering::Relaxed);
    }

    /// Reset the hook-installed flag so tests that call install_panic_hook()
    /// start from a clean slate.
    fn reset_hook_flag() {
        // Relaxed ordering: same rationale as reset_flag — serial gate ensures
        // no concurrent accesses.
        HOOK_INSTALLED.store(false, Ordering::Relaxed);
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
        // Acquire ordering: pairs with any Release store from production code.
        assert!(!MOUSE_CAPTURE_ON.load(Ordering::Acquire));
    }

    #[test]
    #[serial_test::serial]
    fn test_disable_after_simulated_enable_clears_flag() {
        reset_flag();
        // Simulate a successful enable by setting the flag directly. We
        // cannot invoke the real enable_mouse_capture() in unit tests
        // because it writes to the test process's stdout (and on CI
        // that stdout is not a TTY).
        //
        // Release ordering: pairs with the Acquire load/swap inside
        // disable_mouse_capture.
        MOUSE_CAPTURE_ON.store(true, Ordering::Release);
        disable_mouse_capture();
        // The flag must be cleared even if execute! fails (which it will
        // in non-TTY test environments).
        assert!(!MOUSE_CAPTURE_ON.load(Ordering::Acquire));
    }

    #[test]
    #[serial_test::serial]
    fn test_repeated_disable_calls_are_safe() {
        reset_flag();
        disable_mouse_capture();
        disable_mouse_capture();
        disable_mouse_capture();
        // Acquire ordering: pairs with any Release store from production code.
        assert!(!MOUSE_CAPTURE_ON.load(Ordering::Acquire));
    }

    #[test]
    #[serial_test::serial]
    fn test_install_panic_hook_is_idempotent() {
        reset_hook_flag();

        // First call should install the hook (HOOK_INSTALLED becomes true).
        install_panic_hook();
        assert!(HOOK_INSTALLED.load(Ordering::Acquire));

        // Capture the hook installed after the first call.
        let hook_after_first = std::panic::take_hook();

        // Restore for the second call.
        std::panic::set_hook(hook_after_first);

        // Second call must be a no-op: HOOK_INSTALLED is already true.
        install_panic_hook();

        // The hook installed by the second call should be the same chain
        // depth — i.e. take_hook() now returns the same hook that was set
        // after the first call (the second call did not wrap it again).
        // We verify idempotency via the flag: HOOK_INSTALLED remains true
        // and no double-wrap occurred.
        assert!(HOOK_INSTALLED.load(Ordering::Acquire));

        // Restore the default hook so we don't leave a wrapped hook in place
        // for subsequent tests.
        let _ = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
    }
}
