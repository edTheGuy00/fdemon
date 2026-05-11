# Task 03: Emit OSC 22 pointer-shape sequences

**File:** `crates/fdemon-tui/src/terminal.rs`
**Depends on:** Task 04 (Task 04 lands structural changes to `terminal.rs` first; this task is additive)
**Wave:** 1 (Worktree B, sequential after Task 04)

## Background

The terminal's OS-level mouse cursor renders as a text I-beam by default. The
standard fix is OSC 22 (`ESC]22;<shape>ESC\\`), introduced in xterm and
formalized by kitty v0.31.0. Practical support: kitty, xterm, Ghostty, Foot,
opt-in Alacritty. iTerm2, macOS Terminal.app, Windows Terminal, and GNOME
Terminal do not support OSC 22 — they silently discard the sequence
(OSC sequences with unknown numbers are ignored by conforming parsers).

Crossterm has no API for OSC 22. We emit raw bytes via `write!`.

## What to do

1. Add two private byte-string constants at the top of `terminal.rs`:

   ```rust
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
   ```

2. In `enable_mouse_capture()` (around line 79), after the `execute!` call
   succeeds and **before** the `MOUSE_CAPTURE_ON.store(true, ...)` line, emit
   the pointer-shape request. Errors are logged at `warn` and swallowed —
   the pointer is a polish item; capture itself must continue to be reported
   successful.

   ```rust
   if let Err(e) = stdout().write_all(OSC22_POINTER_DEFAULT) {
       warn!("failed to set OSC 22 pointer shape: {e}");
   }
   ```

   Add `use std::io::Write;` at the top of the file if not already imported.

3. In `disable_mouse_capture()` (around line 106), **before** the
   `DisableMouseCapture` execute! call, emit the reset sequence. Same
   error-swallowing policy.

   ```rust
   let _ = stdout().write_all(OSC22_POINTER_RESET);
   ```

   The reset must run while the alt screen is still active and raw mode is
   still on — which is guaranteed by the teardown order established in
   Task 04 (`disable_mouse_capture` runs before `ratatui::restore`).

4. Update the `enable_mouse_capture` doc comment to mention OSC 22 emission
   and its best-effort semantics.

## Verification

- `cargo check -p fdemon-tui` compiles.
- Existing serial tests pass:
  - `test_disable_without_enable_is_noop`
  - `test_disable_after_simulated_enable_clears_flag`
  - `test_repeated_disable_calls_are_safe`
- Add a unit test that the constants contain exactly the expected bytes:
  - `test_osc22_pointer_default_byte_sequence`
  - `test_osc22_pointer_reset_byte_sequence`
  (Reason: the unit-test environment is not a TTY, so the full I/O path
  cannot be observed — the byte constants are the contract.)
- `cargo clippy -p fdemon-tui -- -D warnings` passes.
- Manual on kitty (or another OSC 22 supporting terminal): launch fdemon,
  hover over the TUI, observe arrow cursor (not I-beam). Exit fdemon,
  confirm the cursor reverts.
- Manual on iTerm2 / Terminal.app: launch fdemon, confirm no visible
  garbage appears (the unsupported terminal must silently discard the OSC).
