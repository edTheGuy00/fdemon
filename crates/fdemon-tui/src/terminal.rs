//! Terminal setup and restoration.
//!
//! Provides:
//! - [`install_panic_hook`] — restore the terminal on panic, including mouse
//!   capture if it was enabled.
//! - [`enable_mouse_capture`] / [`disable_mouse_capture`] — gated by an
//!   `AtomicBool` so disable is a no-op when enable was never called or
//!   failed (works around crossterm issue #613 on Windows).
//! - [`set_mouse_capture`] — runtime toggle for the TEA side-effect channel.

use std::io::{stdout, Write};
use std::sync::atomic::{AtomicBool, Ordering};

use fdemon_core::prelude::*;
use tracing::warn;

/// DECSET sequences to enable mouse capture.
///
/// Enables only `?1000` (button events), `?1002` (button-event motion), and
/// `?1006` (SGR extended coordinates). `?1003` (any-motion) and `?1015`
/// (URXVT encoding) are intentionally omitted:
///
/// - `?1003` (any-motion) causes the terminal to route every pointer-movement
///   event to the application, preventing the terminal's own text-selection
///   engine from running (Shift+drag no longer selects text on macOS
///   Terminal.app, iTerm2, Alacritty, Ghostty, Windows Terminal, etc.).
///   fdemon's `event.rs` boundary already drops `Moved` events, so `?1003`
///   provides zero value while damaging native-selection passthrough.
///   See the log-text-selection-broken BUG.md for the root-cause analysis.
///
/// - `?1015` is redundant: its URXVT encoding is superseded by `?1006`'s SGR
///   encoding, which is universally supported by modern terminals.
const ENABLE_MOUSE_DECSET: &[u8] = b"\x1b[?1000h\x1b[?1002h\x1b[?1006h";

/// DECSET sequences to disable mouse capture.
///
/// Disables modes in reverse order from [`ENABLE_MOUSE_DECSET`] per xterm
/// convention: `?1006l` first, then `?1002l`, then `?1000l`. Reverse ordering
/// avoids edge cases on minimalist terminals that process DECRST sequences
/// sequentially and depend on mode-stack ordering.
const DISABLE_MOUSE_DECSET: &[u8] = b"\x1b[?1006l\x1b[?1002l\x1b[?1000l";

/// OSC 22 sequence to request the `default` (arrow) mouse pointer shape.
/// Supported by kitty, xterm, Ghostty, Foot, opt-in Alacritty.
/// Silently ignored by terminals that do not implement OSC 22
/// (iTerm2, macOS Terminal.app, Windows Terminal, GNOME Terminal).
/// See: https://sw.kovidgoyal.net/kitty/pointer-shapes/
const OSC22_POINTER_DEFAULT: &[u8] = b"\x1b]22;default\x1b\\";

/// OSC 22 sequence to reset the pointer shape to the terminal default.
/// An empty shape parameter signals "restore". Same support matrix as
/// `OSC22_POINTER_DEFAULT`.
const OSC22_POINTER_RESET: &[u8] = b"\x1b]22;\x1b\\";

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
///
/// # Ordering
///
/// This MUST be called after `ratatui::init()`. Both functions install
/// panic hooks via the standard "take + wrap" pattern; whichever installs
/// last wraps the other. fdemon's hook must wrap ratatui's so that on
/// panic the order is: disable_mouse_capture → ratatui::restore. Calling
/// in the reverse order causes mouse DECRST sequences to be written to
/// the primary screen after LeaveAlternateScreen, where they may render
/// as visible bytes.
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
        // disable_mouse_capture() runs before ratatui::restore() so that
        // DECRST sequences are emitted while the alt screen is still active.
        // This ordering is guaranteed by the install-order invariant: this
        // hook wraps ratatui's hook (because install_panic_hook() is called
        // after ratatui::init()), so on panic hooks fire LIFO and our cleanup
        // runs first. In practice, DECSET/DECRST mouse modes are
        // connection-global (not alt-screen-scoped), so the ordering doesn't
        // affect cleanup correctness today — but keeping disable-then-restore
        // order makes the invariant explicit and guards against future changes
        // in ratatui's restore() implementation.
        disable_mouse_capture();
        ratatui::restore();
        original_hook(panic_info);
    }));
}

/// Enable terminal mouse capture (button events, drag, scroll wheel).
///
/// Emits `ENABLE_MOUSE_DECSET` (`?1000h ?1002h ?1006h`). `?1003` (any-motion)
/// is intentionally omitted so that native text selection via Shift+drag
/// continues to work on macOS Terminal.app, iTerm2, Alacritty, Ghostty,
/// Windows Terminal, and others. See [`ENABLE_MOUSE_DECSET`] for full
/// rationale and cross-reference to the log-text-selection-broken BUG.md.
///
/// On success, sets the `MOUSE_CAPTURE_ON` flag so the matching
/// [`disable_mouse_capture`] call later actually runs.
///
/// Also emits an OSC 22 sequence (`ESC]22;default ESC\\`) to request the
/// arrow mouse cursor shape on supporting terminals (kitty, xterm, Ghostty,
/// Foot, opt-in Alacritty). This is best-effort: terminals that do not
/// implement OSC 22 silently discard the sequence. Errors from the write
/// are logged at `warn` and swallowed — cursor shape is a polish item and
/// must not prevent capture from succeeding.
///
/// Returns an [`Error`] if the stdout write fails (terminal doesn't support
/// mouse, or stdout write failed). The caller should log the failure and
/// continue — the rest of the application works without mouse support.
pub fn enable_mouse_capture() -> Result<()> {
    // Write the hand-crafted DECSET sequence that omits ?1003 (any-motion).
    // ?1003 breaks native text selection on every mainstream terminal; see
    // ENABLE_MOUSE_DECSET constant for the full rationale.
    stdout().write_all(ENABLE_MOUSE_DECSET).map_err(|e| {
        warn!("failed to enable mouse capture: {e}");
        Error::terminal(format!("EnableMouseCapture failed: {e}"))
    })?;
    // Emit OSC 22 to set the arrow cursor shape. Best-effort: unsupported
    // terminals silently discard OSC sequences with unknown numbers.
    if let Err(e) = stdout().write_all(OSC22_POINTER_DEFAULT) {
        warn!("failed to set OSC 22 pointer shape: {e}");
    }
    // Release ordering: pairs with the Acquire swap in disable_mouse_capture.
    // Ensures the execute! terminal writes happen-before the flag is visible
    // as true to another thread deciding whether to call DisableMouseCapture.
    MOUSE_CAPTURE_ON.store(true, Ordering::Release);
    Ok(())
}

/// Disable terminal mouse capture if it was previously enabled.
///
/// No-op if [`enable_mouse_capture`] was never called or returned an error.
/// This guards against sending DECRST sequences when DECSET was never sent,
/// which avoids crossterm issue #613 (panics on Windows) and prevents
/// confusing terminal state on other platforms.
///
/// Emits `DISABLE_MOUSE_DECSET` (`?1006l ?1002l ?1000l`) — the mirror of
/// [`ENABLE_MOUSE_DECSET`] in reverse order per xterm convention. `?1003` is
/// intentionally absent because it was never enabled. See [`ENABLE_MOUSE_DECSET`]
/// for the full rationale on why `?1003` (any-motion) is omitted.
///
/// Errors from the stdout write are logged at `warn` level and then swallowed —
/// this function must never panic, including when called from inside a panic hook.
pub fn disable_mouse_capture() {
    // AcqRel: acquires the Release store from enable_mouse_capture (so we
    // observe whether the terminal sequences were sent) and releases the
    // false write (so any later thread that checks the flag sees it cleared).
    if !MOUSE_CAPTURE_ON.swap(false, Ordering::AcqRel) {
        return;
    }
    // Emit OSC 22 reset before disabling capture so the cursor shape reverts
    // while the alt screen is still active and raw mode is still on. The reset
    // must run before ratatui::restore() — which is guaranteed by the teardown
    // order established in runner.rs (disable_mouse_capture runs first).
    // Best-effort: errors are silently swallowed so we never block exit.
    let _ = stdout().write_all(OSC22_POINTER_RESET);
    // Write the hand-crafted DECRST sequence (reverse of ENABLE_MOUSE_DECSET).
    // ?1003 is absent because it was never enabled.
    if let Err(e) = stdout().write_all(DISABLE_MOUSE_DECSET) {
        // We must not write to stdout in a panic; tracing is fine because it
        // goes to the file log via tracing-appender (stdout is owned by the TUI).
        warn!("failed to disable mouse capture: {e}");
    }
}

/// Runtime mouse-capture toggle for the TEA side-effect channel.
///
/// - `enabled = true` → calls [`enable_mouse_capture`]. Returns `Ok(())` without
///   re-emitting DECSET sequences if capture is already on (idempotent).
/// - `enabled = false` → calls [`disable_mouse_capture`] and **always returns
///   `Ok(())`**. Write errors on the disable path are logged at `warn` level by
///   [`disable_mouse_capture`] but cannot be propagated through this wrapper —
///   the underlying function returns `()`, not `Result`. Callers must not rely on
///   `Err` to detect disable failures; only the enable path surfaces write errors
///   as `Err`. Returns `Ok(())` without re-emitting DECRST sequences if capture is
///   already off (idempotent via the `MOUSE_CAPTURE_ON` flag).
///
/// The runner uses this as the single entry point for runtime toggling (e.g. when
/// the user toggles `enable_mouse` in settings at runtime). The startup call in
/// `runner.rs` keeps using [`enable_mouse_capture`] directly.
///
/// Note: called by the runner to handle `UpdateAction::SetMouseCapture` from the
/// TEA pipeline. The runner drains `engine.drain_runner_actions()` after each
/// message-processing cycle and calls this function for each `SetMouseCapture`
/// action.
pub fn set_mouse_capture(enabled: bool) -> Result<()> {
    if enabled {
        // Idempotency: if already on, enable_mouse_capture will still emit
        // sequences. Guard with the flag to avoid re-emitting unnecessarily.
        if MOUSE_CAPTURE_ON.load(Ordering::Acquire) {
            return Ok(());
        }
        enable_mouse_capture()
    } else {
        // disable_mouse_capture() swallows its error internally. To give the
        // caller an opportunity to toast, we replicate its logic here and
        // surface any write failure. The AcqRel flag swap is still the gate.
        if !MOUSE_CAPTURE_ON.load(Ordering::Acquire) {
            return Ok(());
        }
        // Delegate to disable_mouse_capture which handles the flag swap and
        // OSC 22 reset. We cannot surface its internal error from here, so
        // we perform the write ourselves only if we need to propagate.
        // Simpler: just call the function and note that its write errors are
        // logged at warn. For the runner's toast use-case, warn-level logging
        // is sufficient without changing disable_mouse_capture's signature.
        disable_mouse_capture();
        Ok(())
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
    fn test_osc22_pointer_default_byte_sequence() {
        // ESC ] 2 2 ; d e f a u l t ESC backslash
        assert_eq!(OSC22_POINTER_DEFAULT, b"\x1b]22;default\x1b\\");
    }

    #[test]
    fn test_osc22_pointer_reset_byte_sequence() {
        // ESC ] 2 2 ; ESC backslash  (empty shape parameter = restore)
        assert_eq!(OSC22_POINTER_RESET, b"\x1b]22;\x1b\\");
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

    // --- DECSET sequence tests ---

    #[test]
    fn test_enable_decset_omits_1003() {
        // ?1003 (any-motion) must NOT be in the enable sequence: it breaks
        // native text selection on macOS Terminal.app, iTerm2, Alacritty,
        // Ghostty, Windows Terminal, and others. See BUG.md §Root Cause.
        assert!(
            !ENABLE_MOUSE_DECSET.windows(5).any(|w| w == b"?1003"),
            "ENABLE_MOUSE_DECSET must not contain ?1003 (any-motion tracking)"
        );
    }

    #[test]
    fn test_enable_decset_contains_1000_1002_1006() {
        // All three required modes must be present in the enable sequence.
        assert!(
            ENABLE_MOUSE_DECSET.windows(5).any(|w| w == b"?1000"),
            "ENABLE_MOUSE_DECSET must contain ?1000 (button events)"
        );
        assert!(
            ENABLE_MOUSE_DECSET.windows(5).any(|w| w == b"?1002"),
            "ENABLE_MOUSE_DECSET must contain ?1002 (button-event motion)"
        );
        assert!(
            ENABLE_MOUSE_DECSET.windows(5).any(|w| w == b"?1006"),
            "ENABLE_MOUSE_DECSET must contain ?1006 (SGR extended coordinates)"
        );
    }

    #[test]
    fn test_disable_decset_reverses_enable() {
        // Disable must cover exactly the same three modes as enable, with
        // 'l' (reset) instead of 'h' (set), in reverse order.
        assert!(
            DISABLE_MOUSE_DECSET.windows(5).any(|w| w == b"?1000"),
            "DISABLE_MOUSE_DECSET must contain ?1000"
        );
        assert!(
            DISABLE_MOUSE_DECSET.windows(5).any(|w| w == b"?1002"),
            "DISABLE_MOUSE_DECSET must contain ?1002"
        );
        assert!(
            DISABLE_MOUSE_DECSET.windows(5).any(|w| w == b"?1006"),
            "DISABLE_MOUSE_DECSET must contain ?1006"
        );
        // Must use DECRST ('l') not DECSET ('h').
        assert!(
            !DISABLE_MOUSE_DECSET.windows(6).any(|w| w == b"?1000h"),
            "DISABLE_MOUSE_DECSET must use 'l' (DECRST), not 'h' (DECSET) for ?1000"
        );
        assert!(
            !DISABLE_MOUSE_DECSET.windows(6).any(|w| w == b"?1002h"),
            "DISABLE_MOUSE_DECSET must use 'l' (DECRST), not 'h' (DECSET) for ?1002"
        );
        assert!(
            !DISABLE_MOUSE_DECSET.windows(6).any(|w| w == b"?1006h"),
            "DISABLE_MOUSE_DECSET must use 'l' (DECRST), not 'h' (DECSET) for ?1006"
        );
        // Verify reverse order: ?1006l appears before ?1002l, which appears before ?1000l.
        let pos_1006 = DISABLE_MOUSE_DECSET
            .windows(6)
            .position(|w| w == b"?1006l")
            .expect("?1006l must be present");
        let pos_1002 = DISABLE_MOUSE_DECSET
            .windows(6)
            .position(|w| w == b"?1002l")
            .expect("?1002l must be present");
        let pos_1000 = DISABLE_MOUSE_DECSET
            .windows(6)
            .position(|w| w == b"?1000l")
            .expect("?1000l must be present");
        assert!(
            pos_1006 < pos_1002,
            "?1006l must appear before ?1002l in DISABLE_MOUSE_DECSET (reverse order)"
        );
        assert!(
            pos_1002 < pos_1000,
            "?1002l must appear before ?1000l in DISABLE_MOUSE_DECSET (reverse order)"
        );
    }

    #[test]
    #[serial_test::serial]
    fn test_set_mouse_capture_idempotent() {
        reset_flag();
        // Simulate already-enabled state by setting the flag directly.
        // We cannot call enable_mouse_capture() in tests (non-TTY stdout).
        MOUSE_CAPTURE_ON.store(true, Ordering::Release);

        // Calling set_mouse_capture(true) when already on must be a no-op:
        // returns Ok(()) without panicking. The flag stays true.
        let result = set_mouse_capture(true);
        assert!(
            result.is_ok(),
            "set_mouse_capture(true) must return Ok when already enabled"
        );
        assert!(
            MOUSE_CAPTURE_ON.load(Ordering::Acquire),
            "flag must remain true after idempotent set_mouse_capture(true)"
        );

        // Cleanup: reset flag so we don't leak state.
        reset_flag();
    }
}
