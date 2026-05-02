## Task: Add mouse-capture enable/disable + panic-safe teardown

**Objective**: Add `enable_mouse_capture()` and `disable_mouse_capture()` helpers in `crates/fdemon-tui/src/terminal.rs`, guarded by an `AtomicBool` so `disable` is a no-op when `enable` was never called (avoiding the crossterm #613 Windows panic). Update the panic hook to disable mouse capture before `ratatui::restore()` so a crash never leaves the terminal in raw mode with mouse reporting on.

**Depends on**: None

**Estimated Time**: 2–3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/terminal.rs` — add `MOUSE_CAPTURE_ON: AtomicBool`, `enable_mouse_capture()`, `disable_mouse_capture()`; update `install_panic_hook()` to call `disable_mouse_capture()` before `ratatui::restore()`

**Files Read (Dependencies):**
- *(none)* — this task is self-contained

### Details

`crossterm 0.29::EnableMouseCapture` writes five escape sequences (`?1000h ?1002h ?1003h ?1015h ?1006h`); `DisableMouseCapture` writes them in reverse with `l`. We must:

1. **Track whether enable succeeded** to avoid the disable-without-enable panic on Windows (crossterm issue #613).
2. **Disable in the panic hook** before `ratatui::restore()` so the user's terminal is fully cleaned up on a crash.
3. **Return `Result<()>`** from `enable` so the caller can log a degraded-experience warning if enabling fails (e.g., terminal does not support mouse), without crashing.
4. **Make `disable` infallible.** It returns `()` and swallows errors — a panic-time call must never re-panic, and a normal-shutdown call must never propagate a terminal-IO error past `ratatui::restore()`.

**Replacement contents for `crates/fdemon-tui/src/terminal.rs`:**

```rust
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
```

### Acceptance Criteria

1. `static MOUSE_CAPTURE_ON: AtomicBool` exists at module scope, initialised to `false`.
2. `enable_mouse_capture()` returns `Result<()>`, sets the flag on success, returns an `Error::terminal(...)` on failure.
3. `disable_mouse_capture()` returns `()`, is a no-op when the flag is `false`, clears the flag unconditionally on entry, and logs (at `warn`) but swallows underlying execute errors.
4. Calling `disable_mouse_capture()` without a prior `enable` does not panic — the test `test_disable_without_enable_is_noop` proves this.
5. The panic hook installed by `install_panic_hook()` calls `disable_mouse_capture()` before `ratatui::restore()`.
6. Existing `install_panic_hook()` callers (currently `runner.rs::run_with_project`, `run_with_project_and_dap`, `run`) continue to compile and behave correctly.
7. `cargo check -p fdemon-tui --all-targets` passes.
8. `cargo test -p fdemon-tui terminal` passes — all three new tests (gated by `serial_test::serial` to run in isolation since they touch global state).
9. `cargo clippy -p fdemon-tui --all-targets -- -D warnings` passes.

### Testing

The three unit tests above cover:

- The crossterm #613 guard (`disable` without prior `enable` is a no-op).
- The flag-clear semantic on disable (flag reads `false` afterwards).
- Idempotent repeat calls (no panic, no flag flip-flop).

We deliberately do NOT unit-test the actual `EnableMouseCapture` execute call. It writes to `stdout`, which is not a TTY in CI, so the call would fail with an `IOError`. Manual testing covers the success path:

**Manual smoke test (run after Task 06 lands):**

1. `cargo run -- /path/to/flutter/project` (with `enable_mouse = true` in config)
2. Click anywhere in fdemon → no observable behavior change in Phase 1 (correct).
3. Press Ctrl+C → terminal returns to a usable state, cursor visible, no stuck mouse reporting.
4. Run again, but this time deliberately panic via a debug `panic!("test")` in `run_loop` before exit → terminal still returns to usable state.
5. Set `enable_mouse = false` and restart → wheel scrolls native terminal scrollback (capture not engaged).

### Notes

- **Why `SeqCst`?** Mouse-capture enable/disable is rare (twice per process lifetime — once at startup, once at shutdown), so the cost of the strongest ordering is negligible and the reasoning stays trivially correct. `Relaxed` would also be sound here but offers no observable benefit.
- **Why `swap` + check?** Using `swap(false, ...)` returns the *previous* value, letting us atomically test-and-clear in one operation. This means a disable from the panic hook concurrent with a normal-shutdown disable still does the right thing (one returns `true`, runs the execute; the other returns `false`, no-ops).
- **Why log via `tracing` instead of `eprintln!`?** CODE_STANDARDS.md forbids `eprintln!` because stdout/stderr are owned by the TUI. `tracing::warn!` routes to the file-based log via `tracing-appender`, which is the project convention.
- **`serial_test` is already a dev-dependency.** See workspace `Cargo.toml` line 68 — no new dep needed.
- **`install_panic_hook()` ordering.** ratatui's own `init()` may install its own panic hook later; whichever runs first on panic is fine, since both call `ratatui::restore()` (idempotent) and our `disable_mouse_capture()` is also idempotent. The user's existing call site (in `runner.rs`) calls `install_panic_hook()` *before* `ratatui::init()`, so our hook wraps theirs and runs second on panic. The ordering does not matter for correctness — only that *both* run.

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/terminal.rs` | Full rewrite: added `MOUSE_CAPTURE_ON: AtomicBool`, `enable_mouse_capture() -> Result<()>`, `disable_mouse_capture()`, updated `install_panic_hook()` to call `disable_mouse_capture()` before `ratatui::restore()`, added 3 unit tests gated with `serial_test::serial` |
| `crates/fdemon-tui/Cargo.toml` | Added `serial_test.workspace = true` to `[dev-dependencies]` (was missing despite being needed by the new tests) |

### Notable Decisions/Tradeoffs

1. **`#[allow(dead_code)]` on `enable_mouse_capture`**: Clippy's `-D warnings` flag fires on the public function since no crate calls it yet (Task 06 will wire it up). Added `#[allow(dead_code)]` with a comment to suppress it cleanly until then. The alternative (removing the annotation) would fail the acceptance criterion that clippy passes.

2. **`serial_test` added to `fdemon-tui` dev-deps**: The task notes say "already a dev-dependency" referring to the workspace Cargo.toml, but `fdemon-tui`'s own Cargo.toml did not list it. Added it there so the `serial_test::serial` attribute compiles.

### Testing Performed

- `cargo check -p fdemon-tui --all-targets` - Passed
- `cargo test -p fdemon-tui terminal` - Passed (39 tests, 3 new: `test_disable_without_enable_is_noop`, `test_disable_after_simulated_enable_clears_flag`, `test_repeated_disable_calls_are_safe`)
- `cargo clippy -p fdemon-tui --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **`enable_mouse_capture` not yet called**: The function is implemented but not connected to any call site. Task 06 will wire it to the runner based on the `enable_mouse` config setting.
