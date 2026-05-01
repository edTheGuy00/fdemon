## Task: Enable / disable mouse capture in the TUI runners

**Objective**: Call `terminal::enable_mouse_capture()` after `ratatui::init()` (gated on `settings.ui.enable_mouse`) and `terminal::disable_mouse_capture()` before each `ratatui::restore()` in the two production TUI entry paths: `run_with_project` and `run_with_project_and_dap`. Leave `run()` (demo) and `selector::select_project` alone — they are out of scope for Phase 1.

**Depends on**: 05-mouse-capture-lifecycle (Task 03 must also have landed for `settings.ui.enable_mouse` to exist, but that is a logical dependency, not a code-edit dependency — `runner.rs` reads from `engine.settings.ui.enable_mouse` which simply must exist by the time this task runs.)

**Estimated Time**: 1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/runner.rs` — wire `enable_mouse_capture()` / `disable_mouse_capture()` into both `run_with_project` and `run_with_project_and_dap`

**Files Read (Dependencies):**
- `crates/fdemon-tui/src/terminal.rs` (Task 05) — for the new helpers
- `crates/fdemon-app/src/config/types.rs` (Task 03) — for the `settings.ui.enable_mouse` field

### Details

The relevant section of each runner today (illustrative — actual order varies):

```rust
let mut term = ratatui::init();
// ... startup, render first frame, dispatch ...
let result = run_loop(&mut term, &mut engine);
engine.shutdown().await;
ratatui::restore();
result
```

We need to:

1. Insert `terminal::enable_mouse_capture()` immediately after `ratatui::init()`, gated on `engine.settings.ui.enable_mouse`. If enable fails, `warn!` and continue (the terminal works without mouse).
2. Insert `terminal::disable_mouse_capture()` immediately before each `ratatui::restore()` in the same path. The `AtomicBool` guard in Task 05 makes this safe even if enable was skipped or failed.

#### Step 1: Update `run_with_project`

Around line 30 of `crates/fdemon-tui/src/runner.rs`:

```rust
let mut term = ratatui::init();

// Enable mouse capture if the user has it on (default true). Failures are
// logged and ignored so the rest of the TUI still works.
if engine.settings.ui.enable_mouse {
    if let Err(e) = terminal::enable_mouse_capture() {
        tracing::warn!("mouse capture disabled: {e}");
    }
}
```

Around line 51 (before `ratatui::restore()`):

```rust
// Disable mouse capture before restoring the terminal so the user's shell
// does not inherit raw mouse-reporting state. Safe to call unconditionally
// — the AtomicBool guard makes it a no-op when capture was never enabled.
terminal::disable_mouse_capture();

// Restore terminal (TUI-specific)
ratatui::restore();
```

#### Step 2: Update `run_with_project_and_dap`

The structure mirrors `run_with_project`. Repeat the same two insertions: enable after `ratatui::init()` (line 110-ish), disable before `ratatui::restore()` (line 134-ish).

#### Step 3: Leave `run()` alone

`runner::run()` is a demo / test entry that creates a dummy engine and does not have user-meaningful settings. Phase 1 does not enable mouse there. (Phase 2+ may revisit if scroll-wheel becomes useful in the demo path.)

Add a one-line code comment explaining the deliberate skip:

```rust
// Demo mode does not enable mouse capture — settings are dummy values
// and the path is not a user-facing entry point.
let mut term = ratatui::init();
```

### Acceptance Criteria

1. After running `run_with_project` or `run_with_project_and_dap` with default settings (`enable_mouse = true`), mouse-capture escape sequences are emitted to stdout (verifiable by running fdemon attached to a `script(1)`-recording session and inspecting the output, or visually by clicking and confirming events arrive — but in Phase 1 they are silently consumed).
2. After running with `enable_mouse = false`, mouse capture is NOT enabled. Verifiable by:
   - Native terminal text selection works without `Shift+drag`.
   - The wheel scrolls the host terminal's scrollback, not anything inside fdemon.
3. Normal exit (`q` → quit) restores the terminal cleanly; cursor visible, no stuck mouse reporting.
4. Ctrl+C / panic exit restores the terminal cleanly via the panic hook.
5. `cargo check -p fdemon-tui --all-targets` passes.
6. `cargo test -p fdemon-tui` passes.
7. `cargo clippy -p fdemon-tui --all-targets -- -D warnings` passes.
8. CI green on Linux, macOS, Windows.

### Testing

Unit-testing the runner end-to-end requires a real terminal, which CI does not provide. The deterministic part is already covered by Task 05's `AtomicBool` tests. This task gets coverage from:

1. **Compile-only checks** — `cargo check` confirms the new calls type-check against the helpers introduced in Task 05.
2. **Manual smoke test** (mandatory before signing off Phase 1):
   - `cargo run --` in a Flutter project on macOS (default `enable_mouse = true`):
     - Click → no behavior change, no crash.
     - Wheel → no behavior change, no scroll inside fdemon (Phase 2 wires that).
     - `q` exit → terminal usable, cursor visible.
     - Re-launch, then `Ctrl+C` → terminal usable.
     - Re-launch, then trigger a `panic!` in `run_loop` (manual code edit for the test) → terminal usable.
   - Set `enable_mouse = false` in `.fdemon/config.toml`:
     - Re-launch → click does whatever the host terminal does; wheel scrolls native scrollback.
3. **Regression check on existing tests** — none of `runner.rs`'s existing behavior is changed for keyboard-only flows; `cargo test --workspace` must still pass.

### Notes

- **The `tracing::warn!` import.** `runner.rs` already uses `tracing::error!` (line 11). Add `warn` to the existing `use tracing::error;` line, or use a fully qualified `tracing::warn!(...)` call to avoid touching imports — pick whichever produces cleaner diff.
- **Don't `?` the enable result.** Mouse failure must not abort startup. `if let Err(e) = ... { warn!(...); }` is the correct pattern.
- **Don't enable mouse before render-first-frame.** It is fine either way (the `?1000h ?1006h` sequence does not interfere with the alternate screen), but enabling immediately after `ratatui::init()` and before any draw keeps the order easy to reason about: init → enable → draw.
- **Settings are read once at startup.** Toggling `enable_mouse` via the settings panel during a session has no immediate effect; the description "Restart required" (set in Task 03) communicates this to users.
- **`selector.rs` deliberately untouched.** The project selector runs *before* the engine exists, has no settings, and is short-lived. Adding mouse there is a Phase 5 stretch goal at most. Not in this task.
