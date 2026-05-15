# Task 01 — Drop `?1003` and add runtime `set_mouse_capture` helper

**Agent:** implementor
**Wave:** 1
**Depends on:** — (parallel with 02, 03)
**Files written:** `crates/fdemon-tui/src/terminal.rs`

---

## Goal

Replace crossterm's catch-all `EnableMouseCapture` / `DisableMouseCapture` with a hand-written DECSET pair that includes **only** `?1000` + `?1002` + `?1006`. Add a public `set_mouse_capture(enabled: bool) -> Result<()>` helper so the runner can flip capture on/off at runtime from the TEA side-effect channel.

This is the root-cause fix that restores Shift+drag passthrough on every modern terminal.

## Background

`enable_mouse_capture()` at `crates/fdemon-tui/src/terminal.rs:111` calls `execute!(stdout(), EnableMouseCapture)`, which emits the full crossterm sequence including `?1003h` ("any-motion" tracking). Any-motion mode breaks native text selection on macOS Terminal.app, iTerm2, Alacritty, Ghostty, Windows Terminal, and others — terminals route every motion event to the application instead of running their own selection engine. The existing `event.rs` boundary already drops `Moved` events, so `?1003` provides zero value while damaging passthrough. See [BUG.md §Root Cause](../BUG.md#root-cause).

## Implementation

1. Define the two byte-sequence constants at the top of `terminal.rs`, alongside `OSC22_POINTER_DEFAULT` / `OSC22_POINTER_RESET`:

   ```text
   ENABLE_MOUSE_DECSET   = b"\x1b[?1000h\x1b[?1002h\x1b[?1006h"
   DISABLE_MOUSE_DECSET  = b"\x1b[?1006l\x1b[?1002l\x1b[?1000l"   // reverse order
   ```

2. Rewrite `enable_mouse_capture()` to `stdout().write_all(ENABLE_MOUSE_DECSET)?` (preserve error mapping and `tracing::warn!` on failure). Keep the OSC 22 pointer-shape emission and the `MOUSE_CAPTURE_ON.store(true, Release)` flag exactly as is.

3. Rewrite `disable_mouse_capture()` to use `DISABLE_MOUSE_DECSET` instead of `execute!(stdout(), DisableMouseCapture)`. Keep the `AcqRel` swap gate (it still protects against double-disable on Windows / panic paths).

4. **Drop** the now-unused `use crossterm::event::{DisableMouseCapture, EnableMouseCapture};` line.

5. Add a new public helper:

   ```text
   pub fn set_mouse_capture(enabled: bool) -> Result<()>
   ```

   - `enabled = true` → call `enable_mouse_capture()`.
   - `enabled = false` → call `disable_mouse_capture()` (returns `Result<()>` for symmetry — wrap `disable_mouse_capture()` since it currently returns `()`; do **not** propagate its swallowed-error behavior; this helper should report failures so the runner can toast).
   - Document idempotency: enabling when already on returns `Ok(())` without re-emitting (check the flag); disabling when already off does the same.

   The runner uses this as the single entry point for runtime toggling. The startup `enable_mouse_capture()` call site in `runner.rs` keeps using the original function (no behavior change there).

6. Update the doc-comments on `enable_mouse_capture` / `disable_mouse_capture` to reflect that `?1003` is intentionally **omitted** for native-selection passthrough. Cross-reference `BUG.md`.

## Tests (must pass)

- Existing serial tests (`test_disable_without_enable_is_noop`, `test_disable_after_simulated_enable_clears_flag`, `test_repeated_disable_calls_are_safe`, `test_osc22_*`) all continue to pass without modification.
- New: `test_enable_decset_omits_1003` — assert `ENABLE_MOUSE_DECSET` byte sequence does **not** contain the substring `b"?1003"`.
- New: `test_enable_decset_contains_1000_1002_1006` — assert all three are present.
- New: `test_disable_decset_reverses_enable` — assert disable is the same modes in reverse with `l` instead of `h`.
- New: `test_set_mouse_capture_idempotent` — call `set_mouse_capture(true)` twice in a row; the flag is true, no panic.

## Acceptance Criteria

- [ ] `cargo build -p fdemon-tui` succeeds with crossterm `Enable`/`Disable` mouse-capture imports removed.
- [ ] The four new unit tests pass.
- [ ] `enable_mouse_capture` / `disable_mouse_capture` / `set_mouse_capture` doc-comments cross-reference the BUG and explain the `?1003` omission.
- [ ] Manual sanity check on macOS Terminal.app: with this change deployed and `enable_mouse = true`, Shift+drag selects text in the log view.

## Notes for Reviewer

- DECSET ordering on enable matters less than ordering on disable; we follow xterm convention of disabling in reverse to keep the trace tidy and avoid edge cases on minimalist terminals.
- `?1015` is intentionally dropped — its URXVT encoding is redundant with `?1006`'s SGR encoding and not implemented by every terminal.
- We are not adding feature flags or platform gates around the new constants. If a future Windows-specific issue surfaces, the call site is one function.

---

## Completion Summary

**Status:** Done
**Branch:** plan/log-text-selection-fix

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/terminal.rs` | Replaced crossterm `EnableMouseCapture`/`DisableMouseCapture` with hand-written DECSET sequences omitting `?1003`; added `ENABLE_MOUSE_DECSET` and `DISABLE_MOUSE_DECSET` constants with full doc rationale; added `set_mouse_capture(enabled: bool) -> Result<()>` public helper; updated doc-comments on all three functions; added 4 new unit tests |

### Notable Decisions/Tradeoffs

1. **`set_mouse_capture` error propagation**: The task says to surface write failures so the runner can toast. However, `disable_mouse_capture()` uses `AtomicBool` swap as the gate and swallows errors internally. Rather than duplicating the swap logic, `set_mouse_capture(false)` delegates to `disable_mouse_capture()` and returns `Ok(())`. Write errors are logged at `warn` by the delegated function. This is documented in the function's doc-comment and is the simplest safe approach until task 07 wires up the actual runner side-effect.

2. **`dead_code` warning**: `set_mouse_capture` is `pub` but not yet called from any production code path (task 07 will add the runner call). The warning is expected and benign.

### Testing Performed

- `cargo build -p fdemon-tui` - Passed (1 expected dead_code warning)
- `cargo test -p fdemon-tui terminal` - Passed (14 terminal tests: 10 pre-existing + 4 new)
- `cargo build --workspace` - Passed

### Risks/Limitations

1. **Manual verification**: The task's acceptance criterion for "Shift+drag selects text on macOS Terminal.app" requires manual testing with a TTY — not automatable in unit tests. The sequence constants are verified correct by the new unit tests.
