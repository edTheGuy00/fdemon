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

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support (worktree-agent-a3984e7ed0387cdbd)

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/event.rs` | Added `drain_input(timeout: Duration)` helper function and `test_drain_input_returns_quickly_with_no_pending_events` unit test (marked `#[ignore]` for non-TTY CI) |
| `crates/fdemon-tui/src/runner.rs` | Reordered teardown in all three entry points (`run_with_project`, `run_with_project_and_dap`, `run`): moved `install_panic_hook()` to after `ratatui::init()`, moved `disable_mouse_capture()` and added `drain_input(50ms)` before `engine.shutdown().await` |
| `crates/fdemon-tui/src/terminal.rs` | Added `# Ordering` section to `install_panic_hook` doc comment; updated hook closure comment to reflect install-order invariant |

### Notable Decisions/Tradeoffs

1. **`drain_input` placed before the existing `poll()` function in `event.rs`**: Follows module organization — all public I/O-adjacent functions grouped together.
2. **`#[ignore]` on `test_drain_input_returns_quickly_with_no_pending_events`**: CI environments have non-TTY stdin where `crossterm::event::poll` behaviour is unpredictable; ignoring avoids false failures while preserving the test for local TTY runs.
3. **`run()` demo entry point also updated**: Although it doesn't enable mouse capture, moving `install_panic_hook()` after `ratatui::init()` keeps the pattern consistent across all three entry points.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check -p fdemon-tui` - Passed
- `cargo test -p fdemon-tui` - Passed (1007 tests, 1 ignored)
- `cargo clippy -p fdemon-tui -- -D warnings` - Passed

### Risks/Limitations

1. **`drain_input` is best-effort**: If the terminal takes more than 50ms to flush buffered events after `DisableMouseCapture`, residual bytes could still leak. 50ms is well above typical terminal latency in practice.
2. **Idempotency guard in `install_panic_hook`**: Because the guard uses a global `AtomicBool`, the ordering fix only applies when `install_panic_hook()` is called for the first time. If called before `ratatui::init()` from some external path, the guard would prevent the post-init wrapping. Current callers are all in runner entry points, which now consistently call after init.
