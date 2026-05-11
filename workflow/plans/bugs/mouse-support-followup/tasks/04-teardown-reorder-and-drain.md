# Task 04: Reorder teardown, move panic-hook install, add `drain_input` helper

**Files:** `crates/fdemon-tui/src/runner.rs`, `crates/fdemon-tui/src/event.rs`, `crates/fdemon-tui/src/terminal.rs`
**Depends on:** None
**Wave:** 1 (Worktree B, lands before Task 03)

## Background

Two distinct issues cause SGR mouse sequences to leak into the shell after
fdemon exits:

- **3a — Buffered mouse events during async shutdown.** In `runner.rs:56-69`
  (and the mirror at `148-162`), `terminal::disable_mouse_capture()` runs
  AFTER `engine.shutdown().await`. Shutdown is async and can take 100s of
  ms. During that window mouse capture is still active; mouse movements
  generate SGR reports that accumulate in the kernel TTY queue. When fdemon
  exits, the shell reads those bytes and prints them.
- **3b — Panic-hook ordering.** `install_panic_hook()` is called at line 24,
  BEFORE `ratatui::init()` at line 30. Both use the standard
  "take + wrap" set_hook pattern, so ratatui's hook ends up wrapping
  fdemon's. On panic, hooks fire LIFO → ratatui's `restore()` runs FIRST
  (leaving the alt screen), then fdemon's `disable_mouse_capture()` fires
  and writes DECRST sequences to the **primary screen** where they may
  render as visible bytes.

## What to do

### 1. Add a `drain_input` helper

In `crates/fdemon-tui/src/event.rs` (or the appropriate event module — confirm
the existing module path), add:

```rust
use std::time::{Duration, Instant};

/// Drain pending terminal events for up to `timeout`, discarding them.
///
/// Used during exit to consume any mouse SGR reports that the terminal
/// emitted before `DisableMouseCapture` took effect — without draining,
/// those reports remain in the kernel TTY queue and leak to the shell
/// after fdemon exits.
///
/// Returns when no event is available within a single poll slice or when
/// the cumulative elapsed time exceeds `timeout`. Errors from
/// `crossterm::event::poll` / `read` are silently swallowed — this is
/// best-effort cleanup; we must not block exit indefinitely.
pub fn drain_input(timeout: Duration) {
    let start = Instant::now();
    while start.elapsed() < timeout {
        let remaining = timeout.saturating_sub(start.elapsed());
        // Cap each poll slice so a stuck terminal cannot block the full timeout.
        let slice = remaining.min(Duration::from_millis(10));
        match crossterm::event::poll(slice) {
            Ok(true) => {
                let _ = crossterm::event::read();
            }
            Ok(false) => return,
            Err(_) => return,
        }
    }
}
```

If the `event` module's existing functions live elsewhere (e.g. a
`pub fn poll()` wrapper), keep this helper alongside them. Confirm the
import path matches what the runner currently uses.

### 2. Reorder teardown in `run_with_project` (`runner.rs:22-70`)

Current order:

```rust
terminal::install_panic_hook();           // line 24  ← BEFORE ratatui::init
let mut engine = Engine::new(...);        // 27
let mut term = ratatui::init();           // 30
if engine.settings.ui.enable_mouse { ... terminal::enable_mouse_capture() ... }  // 34-38
// ... startup, dispatch, run_loop ...
let result = run_loop(...);               // 56
engine.shutdown().await;                  // 59
terminal::disable_mouse_capture();        // 64  ← AFTER shutdown
ratatui::restore();                       // 67
result
```

New order:

```rust
let mut engine = Engine::new(...);
let mut term = ratatui::init();
terminal::install_panic_hook();           // AFTER ratatui::init so we wrap its hook
if engine.settings.ui.enable_mouse { ... terminal::enable_mouse_capture() ... }
// ... startup, dispatch, run_loop ...
let result = run_loop(...);
terminal::disable_mouse_capture();        // FIRST: stop terminal from generating new mouse events
event::drain_input(std::time::Duration::from_millis(50));  // Consume already-buffered mouse reports
engine.shutdown().await;                  // Now safe — no new SGR sequences will queue
ratatui::restore();
result
```

### 3. Apply the same reorder to `run_with_project_and_dap` (`runner.rs:87-163`)

The structure of this function mirrors `run_with_project`. Apply the same
two changes:

- Move `install_panic_hook()` from line 93 to immediately after
  `ratatui::init()` at line 123 (between init and `enable_mouse_capture`).
- Move `disable_mouse_capture()` + add `drain_input(50ms)` to run BEFORE
  `engine.shutdown().await` at line 152.

### 4. Update `terminal.rs` doc comments

Update the `install_panic_hook` doc comment (around line 31) to add a
warning paragraph:

```text
/// # Ordering
///
/// This MUST be called after `ratatui::init()`. Both functions install
/// panic hooks via the standard "take + wrap" pattern; whichever installs
/// last wraps the other. fdemon's hook must wrap ratatui's so that on
/// panic the order is: disable_mouse_capture → ratatui::restore. Calling
/// in the reverse order causes mouse DECRST sequences to be written to
/// the primary screen after LeaveAlternateScreen, where they may render
/// as visible bytes.
```

Update the comment block at `terminal.rs:55-61` inside the hook closure to
reflect that the install-order invariant (not just the comment) now
guarantees correctness.

The `run()` demo entry point at `runner.rs:165-186` does not enable mouse
capture, so no reorder is needed there — but the `install_panic_hook()`
call at line 167 should also move to AFTER `ratatui::init()` at line 175
for consistency.

## Verification

- `cargo check -p fdemon-tui` compiles.
- `cargo test -p fdemon-tui` passes. Existing tests
  (`test_install_panic_hook_is_idempotent`,
  `test_disable_without_enable_is_noop`, etc.) must still pass — the
  reorder changes call sites, not behaviour of the individual helpers.
- Add unit test for `drain_input`:
  - `test_drain_input_returns_quickly_with_no_pending_events` — call with
    `Duration::from_millis(100)`; assert returns in under 50ms when no
    events are available. (Note: needs to run in an environment where
    `crossterm::event::poll` works; gate behind `#[cfg(not(ci))]` or
    `#[ignore]` if stdin is not a TTY in CI.)
- `cargo clippy -p fdemon-tui -- -D warnings` passes.
- Manual: launch fdemon, move mouse vigorously, press Q. Confirm no
  garbage appears in the shell after exit.
- Manual: launch fdemon, trigger a panic (e.g. an unexpected state path,
  or temporarily insert `panic!()` in dev). Confirm the terminal is
  restored cleanly without DECRST bytes leaking to the primary screen.
